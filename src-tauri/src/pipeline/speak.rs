/*!
 * SOURCE OF TRUTH KEYWORDS: SpeakControl, resolve_output_device, set_safe_mode,
 *   toggle_safe_mode, speak_text, meeting_safe_mode, voice_mode
 * WHAT:  Voice mode's brain: speaks a finished dictation into a meeting, owns
 *        the meeting-safe-mode switch, and publishes what the speaking side is
 *        doing.
 * WHY:   The rule that matters is what happens when speech goes wrong. In a
 *        meeting, a voice that errors or arrives late is not a bug to log, it is
 *        dead air with people watching — so failure is answered, not just
 *        reported: unless the person turned it off, the real microphone is let
 *        through (safe mode) at once and the reason is published so they see why
 *        they are suddenly live. A speech that was cut off ON PURPOSE (a newer
 *        one replaced it) is not a failure and never trips it.
 *
 *        Safe mode is its own switch, not a flag on speaking: it works when
 *        speaking is broken, which is the only time it is needed.
 *
 *        "Automatic" output means a virtual / loopback device if one exists —
 *        that is what a meeting can take as a microphone — and the speakers
 *        otherwise, so Voice mode degrades to something audible rather than to
 *        nothing.
 * WHERE: One per app, in AppState and SessionContext; called by
 *        session/delivery.rs (speak), ipc/commands/speak.rs (safe mode, test) and
 *        the shortcut handlers.
 */

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;

use crate::error::{AppError, AppResult, ErrorCode};
use crate::ports::events::EventSink;
use crate::ports::speech::SpeechOutput;
use crate::session::SessionSettings;
use crate::telemetry::now_ms;
use crate::types::{AudioOutputDevice, SpeakReport, SpeakRequest, SpeakStatus, SPEECH_CANCELLED};

/// What "Test voice" says.
pub const TEST_SENTENCE: &str = "This is how Vocs will sound in your meeting.";

/**
 * WHAT:  The device a Speak-into setting names, as an id for the adapter (None
 *        meaning the system default).
 * WHY:   A named device that has been unplugged falls back to automatic rather
 *        than to an error: speaking through the speakers is a better answer
 *        than silence the person only discovers mid-sentence.
 */
pub fn resolve_output_device(setting: &str, devices: &[AudioOutputDevice]) -> Option<String> {
    let automatic = || devices.iter().find(|d| d.is_virtual).map(|d| d.id.clone());
    match setting {
        "default" => None,
        "auto" | "" => automatic(),
        named => devices.iter().find(|d| d.id == named).map(|d| d.id.clone()).or_else(automatic),
    }
}

pub struct SpeakControl {
    speech: Arc<dyn SpeechOutput>,
    events: Arc<dyn EventSink>,
    status: Mutex<SpeakStatus>,
}

impl SpeakControl {
    pub fn new(speech: Arc<dyn SpeechOutput>, events: Arc<dyn EventSink>) -> Self {
        Self { speech, events, status: Mutex::new(SpeakStatus::idle(now_ms())) }
    }

    pub fn speech(&self) -> &Arc<dyn SpeechOutput> {
        &self.speech
    }

    pub fn status(&self) -> SpeakStatus {
        self.status.lock().clone()
    }

    /// Applies a change, stamps it and tells the UI — outside the lock, so a slow
    /// listener can never stall the audio path.
    fn publish(&self, change: impl FnOnce(&mut SpeakStatus)) {
        let snapshot = {
            let mut status = self.status.lock();
            change(&mut status);
            status.changed_at = now_ms();
            status.clone()
        };
        self.events.speak_status_changed(snapshot);
    }

    fn request_for(&self, settings: &SessionSettings, text: &str, language: Option<&str>) -> SpeakRequest {
        let devices = self.speech.output_devices();
        SpeakRequest {
            text: text.to_string(),
            voice_id: settings.speak_voice.clone(),
            language: language.map(str::to_string),
            device_id: resolve_output_device(&settings.speak_output_device, &devices),
            rate: settings.speak_rate,
            first_audio_timeout: Duration::from_millis(settings.speak_lag_ms),
        }
    }

    /**
     * WHAT:  Loads the chosen voice while the person is still talking. Blocking.
     * WHY:   The first sentence in a voice measured 1.1 s to first sound against
     *        0.4 s for every later one; the difference is the voice loading. In a
     *        meeting that is the gap before anyone hears you, so it is paid
     *        during the recording, where it costs nobody anything.
     */
    pub fn warm(&self, settings: &SessionSettings, language: Option<&str>) {
        self.speech.warm(&self.request_for(settings, "Ready.", language));
    }

    /// Speaks a dictation. Blocking. On a real failure, lets the microphone
    /// through when the person has asked for that.
    pub fn speak(&self, settings: &SessionSettings, text: &str, language: Option<&str>) -> AppResult<SpeakReport> {
        self.speak_with(settings, text, language, settings.speak_fallback)
    }

    /// Speaks the test sentence. A failed test never switches safe mode on.
    pub fn test_voice(&self, settings: &SessionSettings) -> AppResult<SpeakReport> {
        self.speak_with(settings, TEST_SENTENCE, Some("en"), false)
    }

    fn speak_with(
        &self,
        settings: &SessionSettings,
        text: &str,
        language: Option<&str>,
        fall_back_to_mic: bool,
    ) -> AppResult<SpeakReport> {
        if self.status().safe_mode {
            return Err(AppError::new(
                ErrorCode::SpeechFailed,
                "Meeting safe mode is on, so your microphone is going straight to the meeting. Turn it off to speak again.",
            ));
        }

        let request = self.request_for(settings, text, language);

        self.publish(|status| status.speaking = true);
        let result = self.speech.speak(&request);
        self.publish(|status| status.speaking = false);

        let Err(err) = &result else {
            return result;
        };
        if fall_back_to_mic && err.detail.as_deref() != Some(SPEECH_CANCELLED) {
            tracing::warn!(error = %err, "speaking failed; letting the microphone through");
            let reason = format!("Speaking failed: {}", err.message);
            if let Err(fallback) = self.set_safe_mode(settings, true, Some(reason)) {
                // The person is now neither heard nor speaking; the error they
                // see should say so rather than hide it behind the first one.
                tracing::error!(error = %fallback, "safe mode could not start after speech failed");
            }
        }
        result
    }

    /// Switches meeting safe mode on or off. `reason` is shown when it was not the person's choice.
    pub fn set_safe_mode(&self, settings: &SessionSettings, on: bool, reason: Option<String>) -> AppResult<()> {
        if on {
            let devices = self.speech.output_devices();
            let output = resolve_output_device(&settings.speak_output_device, &devices);
            self.speech.start_passthrough(settings.input_device.as_deref(), output.as_deref())?;
            self.publish(|status| {
                status.speaking = false;
                status.safe_mode = true;
                status.safe_mode_reason = reason;
            });
        } else {
            self.speech.stop_passthrough();
            self.publish(|status| {
                status.safe_mode = false;
                status.safe_mode_reason = None;
            });
        }
        Ok(())
    }

    /// Flips safe mode and returns what it is now.
    pub fn toggle_safe_mode(&self, settings: &SessionSettings) -> AppResult<bool> {
        let turn_on = !self.status().safe_mode;
        self.set_safe_mode(settings, turn_on, None)?;
        Ok(turn_on)
    }

    /// Cuts off anything being said.
    pub fn stop(&self) {
        self.speech.stop_speaking();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::ScriptedSpeech;
    use crate::types::{SpeakStatus as Status, VoiceInfo};

    fn device(name: &str, virtual_: bool) -> AudioOutputDevice {
        AudioOutputDevice { id: name.into(), name: name.into(), is_virtual: virtual_, is_default: !virtual_ }
    }

    #[derive(Default)]
    struct Heard(Mutex<Vec<Status>>);

    impl crate::ports::events::EventSink for Heard {
        fn session_state_changed(&self, _: &crate::types::SessionState) {}
        fn audio_level(&self, _: crate::types::AudioLevel) {}
        fn mic_test_update(&self, _: crate::types::MicTestUpdate) {}
        fn transcript_delivered(&self, _: u32, _: crate::types::DeliveryKind) {}
        fn set_pill_visible(&self, _: bool) {}
        fn caption_changed(&self, _: Option<&crate::types::Caption>) {}
        fn download_progress(&self, _: crate::types::DownloadProgress) {}
        fn model_state_changed(&self, _: crate::types::ModelId, _: crate::types::ModelState) {}
        fn speak_status_changed(&self, status: Status) {
            self.0.lock().push(status);
        }
        fn set_cancel_key_active(&self, _: bool) {}
    }

    fn rig(speech: Arc<ScriptedSpeech>) -> (SpeakControl, Arc<Heard>, SessionSettings) {
        let heard = Arc::new(Heard::default());
        let control = SpeakControl::new(speech, heard.clone());
        (control, heard, SessionSettings::load(&crate::db::Database::open_in_memory().expect("db")))
    }

    #[test]
    fn automatic_prefers_a_virtual_device_and_falls_back_to_the_speakers() {
        let devices = [device("Speakers", false), device("BlackHole 2ch", true)];
        assert_eq!(resolve_output_device("auto", &devices).as_deref(), Some("BlackHole 2ch"));
        assert_eq!(resolve_output_device("auto", &[device("Speakers", false)]), None);
    }

    #[test]
    fn a_named_device_is_used_and_a_vanished_one_falls_back_to_automatic() {
        let devices = [device("Speakers", false), device("BlackHole 2ch", true)];
        assert_eq!(resolve_output_device("Speakers", &devices).as_deref(), Some("Speakers"));
        assert_eq!(resolve_output_device("Unplugged DAC", &devices).as_deref(), Some("BlackHole 2ch"));
        assert_eq!(resolve_output_device("default", &devices), None);
    }

    #[test]
    fn speaking_is_announced_while_it_happens_and_then_ends() {
        let speech = ScriptedSpeech::working(vec![device("BlackHole 2ch", true)]);
        let (control, heard, settings) = rig(speech.clone());

        let report = control.speak(&settings, "Hello there.", Some("en")).expect("speaks");

        assert_eq!(report.first_audio_ms, 120);
        let seen: Vec<bool> = heard.0.lock().iter().map(|s| s.speaking).collect();
        assert_eq!(seen, [true, false]);
        let request = speech.requests.lock()[0].clone();
        assert_eq!(request.device_id.as_deref(), Some("BlackHole 2ch"));
        assert_eq!(request.text, "Hello there.");
    }

    #[test]
    fn a_failed_speech_lets_the_microphone_through_and_says_why() {
        let speech = ScriptedSpeech::failing("Speech took too long to start.", None);
        let (control, heard, settings) = rig(speech.clone());

        assert!(control.speak(&settings, "Hello.", Some("en")).is_err());

        let status = control.status();
        assert!(status.safe_mode, "the meeting must hear the person, not silence");
        assert!(status.safe_mode_reason.as_deref().is_some_and(|r| r.contains("too long")));
        assert!(speech.is_passthrough());
        assert!(heard.0.lock().iter().any(|s| s.safe_mode));
    }

    #[test]
    fn the_fallback_can_be_turned_off() {
        let speech = ScriptedSpeech::failing("The voice produced no sound.", None);
        let (control, _, mut settings) = rig(speech.clone());
        settings.speak_fallback = false;

        assert!(control.speak(&settings, "Hello.", Some("en")).is_err());
        assert!(!control.status().safe_mode);
        assert!(!speech.is_passthrough());
    }

    #[test]
    fn a_speech_cut_off_on_purpose_is_not_a_failure() {
        let speech = ScriptedSpeech::failing("Speech was cancelled.", Some(SPEECH_CANCELLED));
        let (control, _, settings) = rig(speech.clone());

        assert!(control.speak(&settings, "Hello.", Some("en")).is_err());
        assert!(!control.status().safe_mode, "a newer utterance replacing this one must not trip safe mode");
    }

    #[test]
    fn a_failed_voice_test_never_switches_safe_mode_on() {
        let speech = ScriptedSpeech::failing("Speech took too long to start.", None);
        let (control, _, settings) = rig(speech);
        assert!(control.test_voice(&settings).is_err());
        assert!(!control.status().safe_mode);
    }

    #[test]
    fn speaking_is_refused_while_safe_mode_is_on_and_resumes_when_it_ends() {
        let speech = ScriptedSpeech::working(vec![]);
        let (control, _, settings) = rig(speech.clone());

        assert!(control.toggle_safe_mode(&settings).expect("on"));
        assert!(speech.is_passthrough());
        assert!(control.speak(&settings, "Hello.", Some("en")).is_err());
        assert!(speech.requests.lock().is_empty(), "nothing may reach the speech engine in safe mode");

        assert!(!control.toggle_safe_mode(&settings).expect("off"));
        assert!(!speech.is_passthrough());
        assert!(control.speak(&settings, "Hello.", Some("en")).is_ok());
    }

    #[test]
    fn safe_mode_that_cannot_start_is_an_error_and_leaves_the_status_alone() {
        let speech = ScriptedSpeech::without_passthrough();
        let (control, _, settings) = rig(speech);
        assert!(control.set_safe_mode(&settings, true, None).is_err());
        assert!(!control.status().safe_mode);
    }

    #[test]
    fn the_voices_the_speech_engine_lists_are_passed_through_unchanged() {
        let speech = ScriptedSpeech::working(vec![]);
        speech.set_voices(vec![VoiceInfo { id: "v".into(), name: "Me".into(), language: "en-US".into(), is_personal: true }]);
        assert_eq!(speech.voices()[0].name, "Me");
    }
}
