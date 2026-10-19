/*!
 * SOURCE OF TRUTH KEYWORDS: SpeechOutput, speak, start_passthrough, voices,
 *   output_devices, personal_voice_status, voice_mode_port
 * WHAT:  The trait for turning text into speech on a chosen audio device, and for
 *        passing the microphone straight through to that device.
 * WHY:   Both live in one port because they are the two things that can occupy
 *        the same output at the same time, and the adapter has to know about
 *        both to make one yield to the other: speaking while passing the mic
 *        through would put two voices into the meeting. Meeting safe mode is the
 *        passthrough; the port gives it as a first-class operation, not as a
 *        flag on speak, so "my speech failed, let me talk" has no dependency on
 *        the speech path that just failed.
 * WHERE: Implemented by adapters/macos/speech; driven by session/speak.rs.
 */

use crate::error::AppResult;
use crate::types::{AudioOutputDevice, PersonalVoiceStatus, SpeakReport, SpeakRequest, VoiceInfo};

pub trait SpeechOutput: Send + Sync {
    /// Every voice the Mac can speak with; personal voices first when allowed.
    fn voices(&self) -> Vec<VoiceInfo>;

    fn output_devices(&self) -> Vec<AudioOutputDevice>;

    fn personal_voice_status(&self) -> PersonalVoiceStatus;

    /// Shows the system's Personal Voice prompt. The answer arrives through
    /// `done`, because the person may take as long as they like.
    fn request_personal_voice(&self, done: Box<dyn FnOnce(PersonalVoiceStatus) + Send>);

    /// Speaks and returns when the last sound has played, the request failed,
    /// or `stop_speaking` was called. Blocking.
    fn speak(&self, request: &SpeakRequest) -> AppResult<SpeakReport>;

    /// Cuts speech off at once. Safe to call when nothing is speaking.
    fn stop_speaking(&self);

    fn is_speaking(&self) -> bool;

    /// Sends the microphone straight to `device_id` (None: the system default)
    /// until `stop_passthrough`. Replaces any speech in progress.
    fn start_passthrough(&self, device_id: Option<&str>) -> AppResult<()>;

    fn stop_passthrough(&self);

    fn is_passthrough(&self) -> bool;
}
