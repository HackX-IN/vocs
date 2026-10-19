/*!
 * SOURCE OF TRUTH KEYWORDS: MacosSpeech, SpeechOutput, speak, start_passthrough,
 *   voice_mode_adapter, first_audio, safe_mode_engine
 * WHAT:  The SpeechOutput port for macOS: speaks text in a chosen voice into a
 *        chosen audio device, and passes the microphone through to it.
 * WHY:   Speaking is synthesis (AVSpeechSynthesizer, in synth.rs) feeding the
 *        output (cpal, in adapters/cpal) — two halves that meet here. The loop
 *        is deliberately simple and bounded: it waits for FIRST AUDIO up to the
 *        caller's deadline and gives up if none comes, because in a meeting
 *        silence that arrives late is worse than an honest failure the caller
 *        can answer (by letting the microphone through).
 *
 *        Speaking and passthrough are mutually exclusive by construction: starting
 *        either stops the other, so two voices can never reach the meeting at
 *        once. `speak` refuses while passthrough is running instead of queueing,
 *        so a caller that forgot safe mode is on gets an error it can see.
 *
 *        One speech at a time: a second `speak` cancels the first. The newest
 *        thing the person said is the one worth hearing.
 * WHERE: Built by adapters/mod.rs::build_speech; driven by pipeline/speak.
 */

mod synth;
mod voices;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::adapters::cpal::convert::RateConverter;
use crate::adapters::cpal::output::{list_output_devices, OutputBridge};
use crate::adapters::cpal::passthrough::Passthrough;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::ports::speech::SpeechOutput;
use crate::types::{
    AudioOutputDevice, PersonalVoiceStatus, SpeakReport, SpeakRequest, VoiceInfo, SPEECH_CANCELLED,
};

use synth::{Chunk, Synth};

/// Beyond this a single utterance is declared stuck.
const MIN_TOTAL_TIMEOUT: Duration = Duration::from_secs(20);
/// Rough upper bound on how long a word takes to say and play.
const PER_WORD_TIMEOUT: Duration = Duration::from_millis(900);

pub struct MacosSpeech {
    cancel: Arc<AtomicBool>,
    speaking: Arc<AtomicBool>,
    passthrough: Mutex<Option<Passthrough>>,
    /// Held for the length of a `speak`, so two utterances cannot interleave.
    one_at_a_time: Mutex<()>,
}

impl MacosSpeech {
    pub fn new() -> Self {
        Self {
            cancel: Arc::new(AtomicBool::new(false)),
            speaking: Arc::new(AtomicBool::new(false)),
            passthrough: Mutex::new(None),
            one_at_a_time: Mutex::new(()),
        }
    }
}

impl Default for MacosSpeech {
    fn default() -> Self {
        Self::new()
    }
}

fn failed(message: &str, detail: impl std::fmt::Display) -> AppError {
    AppError::new(ErrorCode::SpeechFailed, message).with_detail(detail.to_string())
}

impl SpeechOutput for MacosSpeech {
    fn voices(&self) -> Vec<VoiceInfo> {
        voices::list_voices()
    }

    fn output_devices(&self) -> Vec<AudioOutputDevice> {
        list_output_devices().unwrap_or_default()
    }

    fn personal_voice_status(&self) -> PersonalVoiceStatus {
        voices::personal_voice_status()
    }

    fn request_personal_voice(&self, done: Box<dyn FnOnce(PersonalVoiceStatus) + Send>) {
        voices::request_personal_voice(done);
    }

    fn speak(&self, request: &SpeakRequest) -> AppResult<SpeakReport> {
        if self.passthrough.lock().is_some() {
            return Err(failed(
                "Meeting safe mode is on, so Vocs is passing your microphone through instead of speaking.",
                "passthrough active",
            ));
        }

        // A newer utterance replaces an older one.
        self.cancel.store(true, Ordering::Release);
        let _turn = self.one_at_a_time.lock();
        self.cancel.store(false, Ordering::Release);

        self.speaking.store(true, Ordering::Release);
        let result = self.speak_inner(request);
        self.speaking.store(false, Ordering::Release);
        result
    }

    fn stop_speaking(&self) {
        self.cancel.store(true, Ordering::Release);
    }

    fn is_speaking(&self) -> bool {
        self.speaking.load(Ordering::Acquire)
    }

    fn start_passthrough(&self, input_id: Option<&str>, output_id: Option<&str>) -> AppResult<()> {
        // Anything being said stops first: two voices must not reach the meeting.
        self.stop_speaking();
        let mut slot = self.passthrough.lock();
        if let Some(mut previous) = slot.take() {
            previous.stop();
        }
        *slot = Some(Passthrough::start(input_id, output_id)?);
        Ok(())
    }

    fn stop_passthrough(&self) {
        if let Some(mut running) = self.passthrough.lock().take() {
            running.stop();
        }
    }

    fn is_passthrough(&self) -> bool {
        self.passthrough.lock().is_some()
    }
}

impl MacosSpeech {
    fn speak_inner(&self, request: &SpeakRequest) -> AppResult<SpeakReport> {
        let started = Instant::now();
        let bridge = OutputBridge::open(request.device_id.as_deref())?;
        let words = request.text.split_whitespace().count().max(1) as u32;
        let hard_deadline = started + MIN_TOTAL_TIMEOUT.max(PER_WORD_TIMEOUT * words);

        let (tx, rx) = channel();
        let synth = Synth::start(request, tx)?;

        let mut converter: Option<RateConverter> = None;
        let mut first_audio: Option<Duration> = None;

        loop {
            if self.cancel.load(Ordering::Acquire) {
                synth.stop();
                bridge.clear();
                return Err(failed("Speech was cancelled.", SPEECH_CANCELLED));
            }
            if Instant::now() >= hard_deadline {
                synth.stop();
                return Err(failed("Speech took too long, so Vocs stopped it.", "overall timeout"));
            }

            let wait = match first_audio {
                Some(_) => Duration::from_millis(100),
                None => request.first_audio_timeout.saturating_sub(started.elapsed()).min(Duration::from_millis(100)),
            };
            match rx.recv_timeout(wait) {
                Ok(Chunk::Audio { samples, rate }) => {
                    first_audio.get_or_insert_with(|| started.elapsed());
                    // Built from the first chunk's rate, which a voice keeps for the
                    // whole utterance.
                    if converter.is_none() {
                        converter = Some(RateConverter::new(rate, bridge.sample_rate())?);
                    }
                    let Some(conv) = converter.as_mut() else {
                        continue;
                    };
                    let converted = conv.push(&samples)?;
                    if !bridge.push_blocking(&converted, &self.cancel) {
                        synth.stop();
                        return Err(failed("Speech was cancelled.", SPEECH_CANCELLED));
                    }
                }
                Ok(Chunk::End) => break,
                Err(RecvTimeoutError::Timeout) => {
                    if first_audio.is_none() && started.elapsed() >= request.first_audio_timeout {
                        synth.stop();
                        return Err(failed(
                            "Speech took too long to start.",
                            format!("no audio within {} ms", request.first_audio_timeout.as_millis()),
                        ));
                    }
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }

        if let Some(conv) = converter.as_mut() {
            let tail = conv.flush()?;
            bridge.push_blocking(&tail, &self.cancel);
        }
        let Some(first_audio) = first_audio else {
            return Err(failed("The voice produced no sound.", "empty synthesis"));
        };
        if !bridge.wait_drained(&self.cancel, Duration::from_secs(60)) {
            bridge.clear();
        }

        Ok(SpeakReport { first_audio_ms: first_audio.as_millis() as u64, total_ms: started.elapsed().as_millis() as u64 })
    }
}
