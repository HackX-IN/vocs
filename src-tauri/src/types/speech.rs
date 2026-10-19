/*!
 * SOURCE OF TRUTH KEYWORDS: VoiceInfo, AudioOutputDevice, PersonalVoiceStatus,
 *   SpeakStatus, SpeakReport, SpeakRequest, SpeechMode, safe_mode
 * WHAT:  The vocabulary of Voice mode: the voices the Mac can speak with, the
 *        audio devices it can speak into, whether Personal Voice may be used, and
 *        what the speaking side is doing right now.
 * WHY:   One set of types for the port, the settings pickers and the UI, so the
 *        device a person picks in Settings is the same value the adapter opens —
 *        by its stable UID, never by a name that two devices can share and a
 *        rename can break. `is_virtual` marks a loopback device (BlackHole,
 *        Loopback, a meeting app's own virtual device) because that, not the
 *        speakers, is where Voice mode is meant to speak.
 * WHERE: Produced by adapters/macos/speech; consumed by session/speak.rs,
 *        ipc/commands/speak.rs and Settings → Voice.
 */

use serde::{Deserialize, Serialize};
use specta::Type;

use super::numeric::TsNumber;

/// Whether the person has allowed apps to speak with their Personal Voice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PersonalVoiceStatus {
    /// This Mac or macOS version has no Personal Voice (it needs macOS 14).
    Unsupported,
    NotDetermined,
    Denied,
    Authorized,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VoiceInfo {
    /// The system's identifier, stable across launches.
    pub id: String,
    pub name: String,
    /// BCP-47, e.g. "en-US".
    pub language: String,
    /// The person's own voice, made in Accessibility → Personal Voice.
    pub is_personal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AudioOutputDevice {
    /// The device's UID.
    pub id: String,
    pub name: String,
    /// A loopback / virtual device, the kind a meeting app can take as a mic.
    pub is_virtual: bool,
    pub is_default: bool,
}

/// What to say, in whose voice, and where.
#[derive(Debug, Clone)]
pub struct SpeakRequest {
    pub text: String,
    /// None means the best available voice for the text's language.
    pub voice_id: Option<String>,
    /// None means the system default output.
    pub device_id: Option<String>,
    /// 1.0 is the voice's natural pace.
    pub rate: f32,
}

/// How a spoken delivery went.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeakReport {
    /// From asking to speak until the first sound left, the number a meeting
    /// feels as dead air.
    pub first_audio_ms: u64,
    pub total_ms: u64,
}

/// What the speaking side is doing, pushed to the UI whenever it changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SpeakStatus {
    pub speaking: bool,
    /// The microphone is being passed straight through to the output device.
    pub safe_mode: bool,
    /// Why safe mode came on by itself, when it did; None when the person chose it.
    pub safe_mode_reason: Option<String>,
    #[specta(type = TsNumber)]
    pub changed_at: i64,
}

impl SpeakStatus {
    pub fn idle(now: i64) -> Self {
        Self { speaking: false, safe_mode: false, safe_mode_reason: None, changed_at: now }
    }
}
