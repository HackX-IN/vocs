/*!
 * SOURCE OF TRUTH KEYWORDS: list_voices, personal_voice_status, request_personal_voice,
 *   map_personal_voice_status, is_personal, AVSpeechSynthesisVoice
 * WHAT:  The voices the Mac can speak with, and the permission to use the
 *        person's own (Personal Voice).
 * WHY:   Personal Voice is the point of Voice mode — a meeting hears YOU, not a
 *        stock voice — and it is gated twice: it needs macOS 14, and the person
 *        must have allowed apps to use it. Calling the authorisation API on
 *        macOS 13 is not an error, it is an unrecognised selector and a crash,
 *        so the OS version is checked first and an older Mac reports Unsupported.
 *
 *        Novelty voices (Bells, Zarvox, Whisper) are left out of the list: they
 *        are not what anyone wants in a meeting, and offering them next to the
 *        real ones makes the picker a minefield. Personal voices sort first, then
 *        the rest by name.
 * WHERE: Called by adapters/macos/speech/mod.rs for the SpeechOutput port.
 */

use block2::RcBlock;
use objc2_avf_audio::{
    AVSpeechSynthesisPersonalVoiceAuthorizationStatus, AVSpeechSynthesisVoice, AVSpeechSynthesisVoiceTraits,
    AVSpeechSynthesizer,
};
use objc2_foundation::NSProcessInfo;
use objc2_foundation::NSOperatingSystemVersion;

use crate::types::{PersonalVoiceStatus, VoiceInfo};

/// Personal Voice arrived in macOS 14.
fn personal_voice_exists() -> bool {
    let version = NSOperatingSystemVersion { majorVersion: 14, minorVersion: 0, patchVersion: 0 };
    NSProcessInfo::processInfo().isOperatingSystemAtLeastVersion(version)
}

/// Apple's raw values: 0 not determined, 1 denied, 2 unsupported, 3 authorised.
pub fn map_status(raw: usize) -> PersonalVoiceStatus {
    match raw {
        0 => PersonalVoiceStatus::NotDetermined,
        1 => PersonalVoiceStatus::Denied,
        3 => PersonalVoiceStatus::Authorized,
        _ => PersonalVoiceStatus::Unsupported,
    }
}

pub fn personal_voice_status() -> PersonalVoiceStatus {
    if !personal_voice_exists() {
        return PersonalVoiceStatus::Unsupported;
    }
    // SAFETY: a class method with no arguments, called only on macOS 14+.
    map_status(unsafe { AVSpeechSynthesizer::personalVoiceAuthorizationStatus() }.0)
}

/// Shows the system prompt; `done` gets the answer, on whatever thread Apple chooses.
pub fn request_personal_voice(done: Box<dyn FnOnce(PersonalVoiceStatus) + Send>) {
    if !personal_voice_exists() {
        done(PersonalVoiceStatus::Unsupported);
        return;
    }
    let slot = std::sync::Mutex::new(Some(done));
    let handler = RcBlock::new(move |status: AVSpeechSynthesisPersonalVoiceAuthorizationStatus| {
        if let Some(done) = slot.lock().ok().and_then(|mut guard| guard.take()) {
            done(map_status(status.0));
        }
    });
    // SAFETY: the block is retained by the system for the duration of the request.
    unsafe { AVSpeechSynthesizer::requestPersonalVoiceAuthorizationWithCompletionHandler(&handler) };
}

/// Every non-novelty voice installed, the person's own first.
pub fn list_voices() -> Vec<VoiceInfo> {
    // SAFETY: plain reads of immutable voice descriptors.
    let voices = unsafe { AVSpeechSynthesisVoice::speechVoices() };
    let mut out: Vec<VoiceInfo> = voices
        .iter()
        .filter_map(|voice| {
            let traits = unsafe { voice.voiceTraits() };
            if traits.contains(AVSpeechSynthesisVoiceTraits::IsNoveltyVoice) {
                return None;
            }
            Some(VoiceInfo {
                id: unsafe { voice.identifier() }.to_string(),
                name: unsafe { voice.name() }.to_string(),
                language: unsafe { voice.language() }.to_string(),
                is_personal: traits.contains(AVSpeechSynthesisVoiceTraits::IsPersonalVoice),
            })
        })
        .collect();
    out.sort_by(|a, b| b.is_personal.cmp(&a.is_personal).then_with(|| a.language.cmp(&b.language)).then_with(|| a.name.cmp(&b.name)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apples_raw_statuses_map_to_ours() {
        assert_eq!(map_status(0), PersonalVoiceStatus::NotDetermined);
        assert_eq!(map_status(1), PersonalVoiceStatus::Denied);
        assert_eq!(map_status(2), PersonalVoiceStatus::Unsupported);
        assert_eq!(map_status(3), PersonalVoiceStatus::Authorized);
        assert_eq!(map_status(99), PersonalVoiceStatus::Unsupported, "an unknown value is never read as permission");
    }

    #[test]
    fn this_machine_lists_voices_without_novelty_ones_and_personal_first() {
        let voices = list_voices();
        assert!(!voices.is_empty(), "macOS always ships a voice");
        assert!(voices.iter().all(|v| !v.name.is_empty() && !v.id.is_empty()));
        // No novelty voice leaks into the picker.
        assert!(!voices.iter().any(|v| v.name == "Zarvox" || v.name == "Bells"));
        // Personal voices, if any, come before every other voice.
        if let Some(first_other) = voices.iter().position(|v| !v.is_personal) {
            assert!(voices[first_other..].iter().all(|v| !v.is_personal));
        }
    }
}
