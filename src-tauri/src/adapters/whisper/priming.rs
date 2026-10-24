/*!
 * SOURCE OF TRUTH KEYWORDS: prime_detection_context, should_prime, PRIME_ABOVE_CTX,
 *   stale_audio_ctx, language_detection_cost, auto_language_tail
 * WHAT:  Decides when a state's remembered encoder context is large enough that
 *        an auto-language tail decode should first shrink it, and does the
 *        shrinking with one throwaway decode.
 * WHY:   whisper.cpp detects the language BEFORE it applies the call's own
 *        `audio_ctx` (whisper.cpp: whisper_full_with_state runs
 *        whisper_lang_auto_detect_with_state, and only later assigns
 *        `state->exp_n_audio_ctx`). So detection runs the encoder at whatever
 *        the PREVIOUS decode on that state used. After a full 1500-wide
 *        background chunk that is the whole 30-second window, and measured on
 *        large-v3-turbo-q5_0 a 1.2-second tail then took 1943 ms with the
 *        language left on auto against 224 ms with it pinned.
 *
 *        A throwaway decode at the minimum context costs roughly 200 ms and
 *        leaves the state remembering 256, so the detection pass that follows
 *        encodes a fifth-second-sized window instead. Detection only needs the
 *        opening seconds of speech, so the smaller window does not change what
 *        it concludes. Priming is skipped when the state is already small —
 *        there it would cost more than it saves — and when the language is
 *        pinned, where no detection happens at all.
 * WHERE: Called by adapters/whisper/engine.rs::transcribe before an
 *        auto-language Tail decode.
 */

use std::ffi::c_int;

use whisper_rs::WhisperState;

use crate::error::{AppError, AppResult, ErrorCode};

use super::benchmark::probe_signal;
use super::params::{build_full_params, DecodeProfile};

/// Above this the detection pass costs more than the priming decode that would
/// avoid it (about 1.1 ms per context position against a ~200 ms prime plus
/// the smaller detection that follows).
pub const PRIME_ABOVE_CTX: c_int = 448;

/// Long enough for whisper.cpp to build a mel spectrogram from, short enough
/// that the encoder is at its minimum context.
const PRIME_SECONDS: f32 = 0.5;

/**
 * WHAT:  Whether this decode should be preceded by a priming decode.
 * WHY:   Only a Tail decode with the language left to detect benefits: a
 *        Background chunk runs at the full context regardless, and a pinned
 *        language never reaches the detection pass.
 * WHERE: engine.rs::transcribe.
 */
pub fn should_prime(profile: DecodeProfile, language_is_auto: bool, remembered_ctx: c_int) -> bool {
    profile == DecodeProfile::Tail && language_is_auto && remembered_ctx > PRIME_ABOVE_CTX
}

/**
 * WHAT:  Runs one throwaway minimum-context decode on `state`.
 * WHY:   Language is pinned and `max_tokens` is 1, so it is the mel and a
 *        small encoder and almost no decoding. Its output is discarded.
 * WHERE: engine.rs::transcribe, behind `should_prime`.
 */
pub fn prime_detection_context(state: &mut WhisperState, n_threads: c_int) -> AppResult<()> {
    let audio = probe_signal(PRIME_SECONDS);
    let mut params = build_full_params(DecodeProfile::Tail, audio.len(), Some("en"), None, n_threads, 0.0);
    params.set_max_tokens(1);
    state.full(params, &audio).map_err(|err| {
        AppError::new(
            ErrorCode::TranscriptionFailed,
            "Vocs could not turn that recording into text.",
        )
        .recoverable()
        .with_detail(format!("{err:?}"))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_auto_language_tail_after_a_large_context_is_primed() {
        assert!(should_prime(DecodeProfile::Tail, true, 1500));
        // A pinned language never runs detection.
        assert!(!should_prime(DecodeProfile::Tail, false, 1500));
        // A background chunk runs at the full context whatever it follows.
        assert!(!should_prime(DecodeProfile::Background, true, 1500));
        // Already small: priming would cost more than it saves.
        assert!(!should_prime(DecodeProfile::Tail, true, 256));
        assert!(!should_prime(DecodeProfile::Tail, true, PRIME_ABOVE_CTX));
        assert!(should_prime(DecodeProfile::Tail, true, PRIME_ABOVE_CTX + 1));
    }
}
