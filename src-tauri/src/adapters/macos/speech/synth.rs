/*!
 * SOURCE OF TRUTH KEYWORDS: Synth, Chunk, start_synthesis, pick_voice, utterance_rate,
 *   AVSpeechSynthesizer, writeUtterance, buffer_callback
 * WHAT:  Asks AVSpeechSynthesizer to render text to audio buffers and forwards
 *        them as plain f32 chunks over a channel.
 * WHY:   Rendering to buffers rather than speaking is what lets Vocs choose the
 *        output device: `speakUtterance:` plays through the system's default
 *        output and has no device parameter, while `writeUtterance:toBufferCallback:`
 *        hands over the audio and leaves the routing to us.
 *
 *        Apple delivers those buffers on the MAIN thread (the main queue), and
 *        only while the main run loop is turning — verified: with no run loop the
 *        callback simply never fires. So the callback does the least possible,
 *        copying samples into a Vec and sending it, and everything heavy (rate
 *        conversion, waiting for room in the output ring) happens on the speaking
 *        thread that owns the receiving end. A zero-length buffer is Apple's
 *        end-of-utterance marker.
 *
 *        Rate: AVSpeechUtterance's scale is 0 to 1 with 0.5 the natural pace.
 *        Settings expresses speed as a multiple of natural (1.0), mapped here and
 *        clamped so no setting can ask for something unintelligible.
 * WHERE: Used by adapters/macos/speech/mod.rs.
 */

use std::ptr::NonNull;
use std::sync::mpsc::Sender;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2_avf_audio::{
    AVAudioBuffer, AVAudioPCMBuffer, AVSpeechBoundary, AVSpeechSynthesisVoice, AVSpeechSynthesizer,
    AVSpeechUtterance,
};
use objc2_foundation::NSString;

use crate::error::AppResult;
use crate::types::SpeakRequest;

/// One piece of rendered audio, or the end of the utterance.
pub enum Chunk {
    Audio { samples: Vec<f32>, rate: u32 },
    End,
}

/// AVSpeechUtterance's default pace; 1.0 in settings maps to this.
const NATURAL_RATE: f32 = 0.5;
/// How much a full step in settings moves the scale. 1.6x lands at 0.65.
const RATE_PER_STEP: f32 = 0.25;

/// Settings' multiple-of-natural pace on AVSpeechUtterance's 0..1 scale.
pub fn utterance_rate(multiple: f32) -> f32 {
    (NATURAL_RATE + (multiple - 1.0) * RATE_PER_STEP).clamp(0.3, 0.7)
}

pub struct Synth {
    synthesizer: Retained<AVSpeechSynthesizer>,
    // Held so the block outlives the synthesis; Apple keeps its own retain, but
    // dropping ours early would be a use-after-free if that ever changed.
    _callback: RcBlock<dyn Fn(NonNull<AVAudioBuffer>)>,
}

/// The named voice, else the best for the language, else the system's pick.
fn pick_voice(request: &SpeakRequest) -> Option<Retained<AVSpeechSynthesisVoice>> {
    // SAFETY: class methods taking NSStrings; the result is nullable.
    unsafe {
        if let Some(id) = request.voice_id.as_deref().filter(|id| !id.is_empty() && *id != "auto") {
            if let Some(voice) = AVSpeechSynthesisVoice::voiceWithIdentifier(&NSString::from_str(id)) {
                return Some(voice);
            }
        }
        let language = request.language.as_deref().unwrap_or("en");
        // A bare "en" is not a locale AVSpeechSynthesisVoice knows.
        let tag = if language.len() == 2 { format!("{language}-{}", language.to_uppercase()) } else { language.to_string() };
        AVSpeechSynthesisVoice::voiceWithLanguage(Some(&NSString::from_str(&tag)))
            .or_else(|| AVSpeechSynthesisVoice::voiceWithLanguage(Some(&NSString::from_str("en-US"))))
    }
}

impl Synth {
    /// Starts rendering `request`. Chunks arrive on `tx` until `Chunk::End`.
    pub fn start(request: &SpeakRequest, tx: Sender<Chunk>) -> AppResult<Self> {
        // SAFETY: everything below is the documented AVSpeechSynthesizer sequence;
        // the callback only reads the buffer it is handed, on the thread it is given.
        unsafe {
            let utterance = AVSpeechUtterance::speechUtteranceWithString(&NSString::from_str(&request.text));
            utterance.setRate(utterance_rate(request.rate));
            if let Some(voice) = pick_voice(request) {
                utterance.setVoice(Some(&voice));
            }

            let callback = RcBlock::new(move |buffer: NonNull<AVAudioBuffer>| {
                let Some(pcm) = buffer.as_ref().downcast_ref::<AVAudioPCMBuffer>() else {
                    return;
                };
                let frames = pcm.frameLength() as usize;
                if frames == 0 {
                    let _ = tx.send(Chunk::End);
                    return;
                }
                let rate = pcm.format().sampleRate().round() as u32;
                if let Some(samples) = read_mono(pcm, frames) {
                    let _ = tx.send(Chunk::Audio { samples, rate });
                }
            });

            let synthesizer = AVSpeechSynthesizer::new();
            synthesizer.writeUtterance_toBufferCallback(&utterance, RcBlock::as_ptr(&callback));
            Ok(Self { synthesizer, _callback: callback })
        }
    }

    /// Stops rendering at once.
    pub fn stop(&self) {
        // SAFETY: the documented way to cancel; harmless when finished.
        unsafe { self.synthesizer.stopSpeakingAtBoundary(AVSpeechBoundary::Immediate) };
    }
}

/// Channel 0 of a buffer as f32, whichever sample format the voice rendered in.
///
/// # Safety
/// `pcm` must be a live buffer holding at least `frames` frames.
unsafe fn read_mono(pcm: &AVAudioPCMBuffer, frames: usize) -> Option<Vec<f32>> {
    let floats = pcm.floatChannelData();
    if !floats.is_null() {
        let channel = (*floats).as_ptr();
        return Some(std::slice::from_raw_parts(channel, frames).to_vec());
    }
    let ints = pcm.int16ChannelData();
    if !ints.is_null() {
        let channel = (*ints).as_ptr();
        return Some(
            std::slice::from_raw_parts(channel, frames).iter().map(|s| f32::from(*s) / f32::from(i16::MAX)).collect(),
        );
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_pace_maps_to_apples_default_and_the_extremes_are_clamped() {
        assert!((utterance_rate(1.0) - 0.5).abs() < f32::EPSILON);
        assert!(utterance_rate(1.6) > 0.6 && utterance_rate(1.6) <= 0.7);
        assert_eq!(utterance_rate(10.0), 0.7);
        assert_eq!(utterance_rate(-10.0), 0.3);
    }
}
