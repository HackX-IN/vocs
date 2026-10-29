/*!
 * SOURCE OF TRUTH KEYWORDS: audio_ctx_for, build_full_params, DecodeProfile,
 *   FULL_AUDIO_CTX, MIN_AUDIO_CTX, AUDIO_CTX_HEADROOM, decode_thread_count,
 *   NO_SPEECH_THOLD, ENTROPY_THOLD
 * WHAT:  Builds the `FullParams` for one decode, and computes the encoder
 *        context size from the fragment length.
 * WHY:   Whisper's encoder always runs over a 30-second mel window, so a 1.2s
 *        tail costs the same as a 25s chunk unless `audio_ctx` is shrunk to fit
 *        the actual audio. That single value is the difference between a ~1.4s
 *        final decode and a sub-200ms one, which is the whole p50 < 300ms
 *        budget. It is applied to EVERY chunk unless Core ML is live. It used to
 *        be the tail only, on the theory that interior chunks decode in the
 *        background where latency is invisible. Measured on a real History, it
 *        is not invisible: a full-context interior decode took 2.3s, and a stop
 *        pressed during one made the tail wait behind it (2.6s to paste). The
 *        headroom below keeps the context wider than the audio.
 *        `temperature_inc = 0.0` disables whisper's temperature-fallback loop —
 *        up to six extra decode passes fired without warning, turning a 200ms
 *        decode into 1.4s. `no_context = true` stops decoder state carrying
 *        across chunk boundaries, which is the cause of "it repeated the same
 *        sentence forty times".
 * WHERE: Called by adapters/whisper/engine.rs for every transcribe; the values
 *        come from docs/03-IMPLEMENTATION-NOTES.md §2.1 and §2.2.
 */

use std::ffi::c_int;

use whisper_rs::{FullParams, SamplingStrategy};

use crate::types::{AccuracyMode, AudioChunk, LanguageHint, TARGET_SAMPLE_RATE};

/// The encoder context that corresponds to whisper's full 30-second window.
pub const FULL_AUDIO_CTX: c_int = 1500;
/// Below this the encoder's convolutional front end has too little to work with
/// and accuracy collapses, so short fragments pay for a floor they do not use.
pub const MIN_AUDIO_CTX: c_int = 256;
/// Extra context beyond the fragment's own length. Whisper's own `stream`
/// example keeps a margin here because the mel window is padded and a context
/// sized exactly to the audio clips the final frames.
pub const AUDIO_CTX_HEADROOM: c_int = 128;

/// The temperature of the ONE retry the engine makes when a decode degenerates
/// into a repetition loop (see degenerate.rs). Greedy decoding is deterministic,
/// so retrying at 0.0 would loop identically; a little randomness is what lets
/// the second attempt take a different path out of the rut. It is never used
/// for a first attempt, so the common case keeps temperature 0.0 and its
/// determinism.
pub const RETRY_TEMPERATURE: f32 = 0.4;

/// Segments whose no-speech probability exceeds this are discarded. Paired with
/// the VAD gate upstream; see docs/03 §2.4.
pub const NO_SPEECH_THOLD: f32 = 0.6;

/**
 * SOURCE OF TRUTH KEYWORDS: ENTROPY_THOLD
 * WHAT:  whisper.cpp's decoder-entropy gate, left at its upstream default.
 * WHY:   docs/03 §2.4 lists this as a hallucination defence, but it only ever
 *        fires inside whisper.cpp's temperature-fallback loop — and §2.2
 *        disables that loop by pinning `temperature_inc` to 0.0. It is
 *        therefore inert by construction. It is set anyway so the value is not
 *        silently inherited if the fallback is ever re-enabled, and it is
 *        documented here so nobody counts it as live protection.
 * WHERE: Applied in build_full_params; the defence that actually fires is the
 *        per-segment no_speech_probability filter in hallucination.rs.
 */
pub const ENTROPY_THOLD: f32 = 2.4;

/**
 * SOURCE OF TRUTH KEYWORDS: DecodeProfile
 * WHAT:  Whether this decode is the latency-critical tail or a background chunk.
 * WHY:   Kept separate from ChunkKind so the accuracy/latency trade is decided
 *        in one place and is legible at the call site, rather than being read
 *        off a boolean four frames deep.
 * WHERE: Derived from AudioChunk::shrinks_context by `DecodeProfile::for_chunk`.
 */
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeProfile {
    /// Full encoder context. Latency is hidden behind the user still talking.
    Background,
    /// Encoder context shrunk to the fragment. The only decode the user waits on.
    Tail,
}

impl DecodeProfile {
    /**
     * WHAT:  The profile for a chunk, given whether a Core ML encoder is live.
     * WHY:   `coreml_live` forces Background, and it is not a tuning choice —
     *        it prevents silent data loss. When whisper.cpp has a Core ML
     *        encoder loaded it stops building its own convolution graph and
     *        hands the ANE encoder `2 * audio_ctx` mel frames instead. The
     *        `.mlmodelc` is compiled for a fixed 3000-frame input, so a reduced
     *        context feeds it 512 frames, and whisper.cpp neither guards against
     *        this nor reports it: the decode returns an EMPTY transcript and
     *        looks merely fast. Measured on 1.8.3 — a tail decode "completed" in
     *        5ms having produced nothing at all.
     *        So the two optimisations are mutually exclusive, and correctness
     *        wins: with Core ML live every chunk keeps the full 1500.
     * WHERE: adapters/whisper/engine.rs::transcribe, once per chunk.
     */
    pub fn for_chunk(chunk: &AudioChunk, coreml_live: bool) -> Self {
        if chunk.shrinks_context() && !coreml_live {
            Self::Tail
        } else {
            Self::Background
        }
    }
}

/**
 * WHAT:  The encoder context size for a fragment of `sample_count` samples.
 * WHY:   `ceil(seconds / 30 * 1500) + headroom`, clamped. The ratio is exact —
 *        1500 context positions cover 30 seconds — so this is the smallest
 *        encoder that still spans the audio, plus a margin for the padded mel
 *        window. Background decodes never call this; they use FULL_AUDIO_CTX.
 * WHERE: adapters/whisper/engine.rs, tail decodes only.
 */
pub fn audio_ctx_for(sample_count: usize) -> c_int {
    audio_ctx_for_accuracy(sample_count, AccuracyMode::Fast)
}

/**
 * WHAT:  `audio_ctx_for` with the margin and floor of an AccuracyMode.
 * WHY:   The same formula, because the ratio is exact; only the headroom and the
 *        floor are choices, and they are the accuracy dial (types/accuracy.rs).
 *        Fast reproduces the constants above, which a test pins.
 */
pub fn audio_ctx_for_accuracy(sample_count: usize, accuracy: AccuracyMode) -> c_int {
    let seconds = sample_count as f64 / f64::from(TARGET_SAMPLE_RATE);
    let scaled = (seconds / 30.0 * f64::from(FULL_AUDIO_CTX)).ceil();

    // The cast is bounded by the clamp below, so it cannot wrap.
    let requested = if scaled >= f64::from(FULL_AUDIO_CTX) {
        FULL_AUDIO_CTX
    } else if scaled <= 0.0 {
        0
    } else {
        scaled as c_int
    };

    requested
        .saturating_add(accuracy.audio_ctx_headroom())
        .clamp(accuracy.min_audio_ctx(), FULL_AUDIO_CTX)
}

/**
 * WHAT:  Threads for one decode: available parallelism minus two, floor of one.
 * WHY:   docs/03 §2.2 asks for physical cores minus two, leaving headroom for
 *        the CoreAudio realtime thread and the UI. On Apple Silicon there is no
 *        SMT, so `available_parallelism` already reports physical cores — which
 *        is why this is correct here and would not be on an x86 host.
 * WHERE: Read once when the engine is constructed.
 */
pub fn decode_thread_count() -> c_int {
    let logical = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(4);
    let usable = logical.saturating_sub(2).max(1);
    c_int::try_from(usable).unwrap_or(1)
}

/**
 * WHAT:  The encoder context a decode of this profile and length will run at.
 * WHY:   One answer shared by build_full_params, which applies it, and the
 *        state pool, which remembers it: whisper.cpp keeps the last value on the
 *        state and the next language-detection pass runs at it, so the two
 *        must never be computed separately.
 * WHERE: build_full_params, and engine.rs after each decode.
 */
pub fn audio_ctx_for_profile(profile: DecodeProfile, sample_count: usize, accuracy: AccuracyMode) -> c_int {
    match profile {
        DecodeProfile::Background => FULL_AUDIO_CTX,
        DecodeProfile::Tail => audio_ctx_for_accuracy(sample_count, accuracy),
    }
}

/**
 * SOURCE OF TRUTH KEYWORDS: build_full_params
 * WHAT:  Every whisper parameter from docs/03 §2.2, plus the profile-dependent
 *        `audio_ctx` and the optional vocabulary prompt.
 * WHY:   One function so the table has exactly one implementation — a parameter
 *        set assembled at two call sites is a parameter set that disagrees with
 *        itself the first time one is edited. `language` is borrowed for `'a`
 *        because whisper-rs ties the params to the string's lifetime.
 * WHERE: Called per decode by adapters/whisper/engine.rs.
 */
pub fn build_full_params<'a>(
    profile: DecodeProfile,
    sample_count: usize,
    language: Option<&'a str>,
    prompt: Option<&str>,
    n_threads: c_int,
    temperature: f32,
) -> FullParams<'a, 'a> {
    build_full_params_for(profile, sample_count, language, prompt, n_threads, temperature, AccuracyMode::Fast)
}

/**
 * SOURCE OF TRUTH KEYWORDS: build_full_params_for
 * WHAT:  `build_full_params` at a chosen AccuracyMode.
 * WHY:   The original keeps its signature (the benchmark and the live tests build
 *        parameters at the budgeted setting by design); this is the one the
 *        engine calls, so the speed/accuracy choice reaches every decode.
 * WHERE: adapters/whisper/engine.rs::decode_once.
 */
pub fn build_full_params_for<'a>(
    profile: DecodeProfile,
    sample_count: usize,
    language: Option<&'a str>,
    prompt: Option<&str>,
    n_threads: c_int,
    temperature: f32,
    accuracy: AccuracyMode,
) -> FullParams<'a, 'a> {
    // Greedy with best_of = 1 unless the person asked for accuracy: temperature
    // is pinned to 0.0, so sampling more candidates would cost time and return
    // identical text. A beam is different — it explores alternatives
    // deterministically, which is what makes it worth its cost.
    let strategy = match accuracy.beam_size() {
        Some(beam_size) => SamplingStrategy::BeamSearch { beam_size, patience: -1.0 },
        None => SamplingStrategy::Greedy { best_of: 1 },
    };
    let mut params = FullParams::new(strategy);

    params.set_n_threads(n_threads);

    // Never translate. The default bites here: with translate on, every
    // language silently becomes English and the multilingual product is gone.
    params.set_translate(false);

    // NEVER set detect_language here, in either arm. whisper-rs documents it as
    // "the same effect as setting the language to auto or None". In whisper.cpp
    // 1.8.3 it is not: `whisper_full_with_state` detects the language and then
    // does `if (params.detect_language) { return 0; }` — it returns ZERO
    // segments and never transcribes. Setting it alongside a null language,
    // which reads as harmlessly stating the intent twice, silently turns every
    // auto-language dictation into an empty transcript. A null language is the
    // whole instruction: whisper.cpp detects from it and then carries on.
    params.set_language(language);

    // The #1 cause of "it repeated the same sentence forty times".
    params.set_no_context(true);

    // 0.0 for every first attempt. Only the engine's single retry after a
    // repetition loop passes more — see RETRY_TEMPERATURE.
    params.set_temperature(temperature);
    // The latency bomb. See the module WHY.
    params.set_temperature_inc(0.0);
    params.set_entropy_thold(ENTROPY_THOLD);
    params.set_no_speech_thold(NO_SPEECH_THOLD);
    params.set_suppress_blank(true);

    // ── THE ONE DELIBERATE DEPARTURE FROM THE docs/03 §2.2 TABLE ──────────
    //
    // The table says `no_timestamps = true`, on the reasoning that we do not
    // use word timings so skipping them is free speed. Measured on whisper.cpp
    // 1.8.3 against real speech, it is the opposite of free and the opposite of
    // fast. Same 3s utterance, same everything else:
    //
    //     no_timestamps = true   ->  340 ms, 1019 characters
    //     no_timestamps = false  ->  113 ms,   41 characters
    //
    // The 1019 characters are the user's sentence repeated twenty-four times.
    // Timestamp tokens are what let the decoder decide it has reached the end
    // of the audio; without them it keeps decoding into the 30-second silent
    // padding whisper always pads to, and loops. `temperature_inc = 0.0` —
    // correctly pinned above for latency — removes the fallback that would
    // otherwise break the loop, so the two settings are individually defensible
    // and together produce a paste of the same sentence two dozen times.
    // `single_segment` does not help: measured at 318 ms and still looping.
    //
    // Keeping them also gives pipeline/assembler.rs real per-segment spans to
    // de-duplicate the 200ms chunk overlap with, which it otherwise could not.
    params.set_no_timestamps(false);

    // whisper.cpp writes to stdout on every call unless all four are off.
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_special(false);
    params.set_print_timestamps(false);

    params.set_audio_ctx(audio_ctx_for_profile(profile, sample_count, accuracy));

    if let Some(prompt) = prompt {
        params.set_initial_prompt(prompt);
    }

    params
}

/**
 * WHAT:  The whisper language code for a hint, or None to auto-detect.
 * WHERE: Called by engine.rs before build_full_params, so the borrowed code
 *        outlives the params.
 */
pub fn language_code(hint: &LanguageHint) -> Option<&str> {
    match hint {
        LanguageHint::Auto => None,
        LanguageHint::Pinned { language } => Some(language.as_str()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_context_scales_with_duration_and_respects_the_floor() {
        // A 1.2s tail: ceil(1.2 / 30 * 1500) = 60, + 128 = 188, floored to 256.
        assert_eq!(audio_ctx_for(TARGET_SAMPLE_RATE as usize * 12 / 10), 256);

        // 10 seconds: ceil(10 / 30 * 1500) = 500, + 128 = 628.
        assert_eq!(audio_ctx_for(TARGET_SAMPLE_RATE as usize * 10), 628);

        // 30 seconds and beyond can never exceed the model's own context.
        assert_eq!(audio_ctx_for(TARGET_SAMPLE_RATE as usize * 30), FULL_AUDIO_CTX);
        assert_eq!(
            audio_ctx_for(TARGET_SAMPLE_RATE as usize * 600),
            FULL_AUDIO_CTX
        );
    }

    #[test]
    fn the_default_accuracy_is_exactly_the_budgeted_context() {
        // The latency numbers in docs/03 were measured with these two constants;
        // Fast must stay a pure alias for them or the budget quietly stops
        // describing the product.
        assert_eq!(AccuracyMode::Fast.min_audio_ctx(), MIN_AUDIO_CTX);
        assert_eq!(AccuracyMode::Fast.audio_ctx_headroom(), AUDIO_CTX_HEADROOM);
        for seconds in [1, 4, 12, 30] {
            let samples = TARGET_SAMPLE_RATE as usize * seconds;
            assert_eq!(audio_ctx_for_accuracy(samples, AccuracyMode::Fast), audio_ctx_for(samples));
        }
    }

    #[test]
    fn higher_accuracy_never_shrinks_the_context_and_never_exceeds_the_models_own() {
        for seconds in [1, 4, 12, 30, 90] {
            let samples = TARGET_SAMPLE_RATE as usize * seconds;
            let fast = audio_ctx_for_accuracy(samples, AccuracyMode::Fast);
            let balanced = audio_ctx_for_accuracy(samples, AccuracyMode::Balanced);
            let accurate = audio_ctx_for_accuracy(samples, AccuracyMode::Accurate);
            assert!(fast <= balanced && balanced <= accurate, "{seconds}s: {fast} {balanced} {accurate}");
            assert!(accurate <= FULL_AUDIO_CTX);
        }
        // The floor is what a short fragment pays for.
        assert_eq!(audio_ctx_for_accuracy(TARGET_SAMPLE_RATE as usize, AccuracyMode::Accurate), 768);
    }

    #[test]
    fn an_empty_fragment_still_yields_a_legal_context() {
        assert_eq!(audio_ctx_for(0), MIN_AUDIO_CTX);
    }

    #[test]
    fn a_live_coreml_encoder_forces_the_full_context_on_every_chunk() {
        let tail = AudioChunk {
            samples: vec![0.0; 16_000],
            start_ms: 0,
            end_ms: 1000,
            kind: crate::types::ChunkKind::Tail,
        };
        assert_eq!(DecodeProfile::for_chunk(&tail, false), DecodeProfile::Tail);
        // Core ML live: reducing the context would return an empty transcript.
        assert_eq!(
            DecodeProfile::for_chunk(&tail, true),
            DecodeProfile::Background
        );
    }

    #[test]
    fn thread_count_never_reaches_zero() {
        assert!(decode_thread_count() >= 1);
    }
}
