/*!
 * SOURCE OF TRUTH KEYWORDS: AccuracyMode, beam_size, min_audio_ctx,
 *   audio_ctx_headroom, speed_vs_accuracy, from_stored
 * WHAT:  How hard the engine works on each decode: greedy or beam search, and how
 *        much encoder context it keeps beyond the audio's own length.
 * WHY:   Speed and accuracy pull against each other in exactly two places, and a
 *        person should choose where they sit rather than inherit our guess.
 *        Whisper's encoder was trained on a full 30-second window; every decode
 *        here shrinks it to the fragment (the whole latency story, docs/03 §2.1),
 *        and a bigger margin keeps it closer to the conditions it was trained
 *        under. Beam search considers several continuations instead of the single
 *        likeliest word at each step, which is what recovers a word greedy
 *        decoding got wrong. Both cost compute, so the default stays on the
 *        setting the latency budget was measured with; the other two exist for
 *        the person who would rather wait a beat longer for fewer mistakes.
 *
 *        Stored as a plain choice string and parsed here so no caller matches on
 *        strings; an unrecognised value reads as Fast rather than failing a
 *        session.
 * WHERE: Read by session/settings_view.rs, carried to the engine on
 *        TranscribeRequest, applied by adapters/whisper/params.rs.
 */

use std::ffi::c_int;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AccuracyMode {
    /// Greedy decoding on the tightest context. What the latency budget assumes.
    #[default]
    Fast,
    /// Greedy decoding with a wider encoder margin: costs encoder time only.
    Balanced,
    /// Beam search on a wide encoder margin: the fewest mistakes, the slowest.
    Accurate,
}

impl AccuracyMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            AccuracyMode::Fast => "fast",
            AccuracyMode::Balanced => "balanced",
            AccuracyMode::Accurate => "accurate",
        }
    }

    pub fn from_stored(value: &str) -> Self {
        match value {
            "balanced" => AccuracyMode::Balanced,
            "accurate" => AccuracyMode::Accurate,
            _ => AccuracyMode::Fast,
        }
    }

    /// Beam width, or None for greedy decoding.
    pub fn beam_size(&self) -> Option<c_int> {
        match self {
            AccuracyMode::Accurate => Some(5),
            AccuracyMode::Fast | AccuracyMode::Balanced => None,
        }
    }

    /// The smallest encoder context any decode uses, however short the audio.
    pub fn min_audio_ctx(&self) -> c_int {
        match self {
            AccuracyMode::Fast => 256,
            AccuracyMode::Balanced => 512,
            AccuracyMode::Accurate => 768,
        }
    }

    /// Context kept beyond the audio's own length.
    pub fn audio_ctx_headroom(&self) -> c_int {
        match self {
            AccuracyMode::Fast => 128,
            AccuracyMode::Balanced => 256,
            AccuracyMode::Accurate => 384,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_round_trips_through_its_stored_name() {
        for mode in [AccuracyMode::Fast, AccuracyMode::Balanced, AccuracyMode::Accurate] {
            assert_eq!(AccuracyMode::from_stored(mode.as_str()), mode);
        }
    }

    #[test]
    fn an_unrecognised_value_reads_as_the_default_rather_than_failing() {
        assert_eq!(AccuracyMode::from_stored("turbo"), AccuracyMode::Fast);
        assert_eq!(AccuracyMode::from_stored(""), AccuracyMode::Fast);
    }

    #[test]
    fn each_step_up_spends_more_and_only_the_top_uses_beams() {
        let modes = [AccuracyMode::Fast, AccuracyMode::Balanced, AccuracyMode::Accurate];
        for pair in modes.windows(2) {
            assert!(pair[0].min_audio_ctx() < pair[1].min_audio_ctx());
            assert!(pair[0].audio_ctx_headroom() < pair[1].audio_ctx_headroom());
        }
        assert_eq!(AccuracyMode::Fast.beam_size(), None);
        assert_eq!(AccuracyMode::Balanced.beam_size(), None);
        assert!(AccuracyMode::Accurate.beam_size().is_some_and(|beams| beams > 1));
    }
}
