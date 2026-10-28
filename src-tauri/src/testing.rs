/*!
 * SOURCE OF TRUTH KEYWORDS: read_wav_f32, synthesise_speech, testing_support,
 *   speech_fixture, synthetic_voice
 * WHAT:  Test-only helpers for getting real spoken audio into a test.
 * WHY:   Two test modules need identical fixtures — the whisper live tests and
 *        the end-to-end session tests — and a second copy of a WAV parser is
 *        exactly the duplication the SOURCE OF TRUTH discipline exists to
 *        prevent. It lives here rather than in either of them so neither owns
 *        it and both can rely on it.
 *
 *        The parser walks RIFF chunks rather than assuming a 44-byte header.
 *        That is not pedantry: `say` emits a WAV with extra chunks before the
 *        data, so a fixed skip reinterprets header bytes as samples and yields
 *        values far outside [-1, 1]. Downstream that trips the VAD's range
 *        assertion, and it looks like a bug in the audio pipeline rather than
 *        in the test that fed it garbage.
 *
 *        ScriptedGrammar lives here for the same reason: the guarded grammar
 *        pass is tested alone (pipeline/grammar/pass.rs) and through the real
 *        actor (session/e2e_tests.rs), and both need the same model double.
 * WHERE: Used by adapters/whisper/live_tests.rs, session/e2e_tests.rs and
 *        pipeline/grammar/pass.rs.
 */

use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::{Mutex, MutexGuard};

use crate::config::AppPaths;
use crate::ports::TranscriptionEngine;

/**
 * SOURCE OF TRUTH KEYWORDS: synthetic_voice, platform_independent_speech
 * WHAT:  `ms` milliseconds of a vowel-like synthetic voice: harmonics of `f0`
 *        shaped by two formants, with a syllable-rate envelope that never falls
 *        silent, peak-normalised well inside [-1, 1].
 * WHY:   `synthesise_speech` needs macOS `say`, so every test built on it
 *        silently SKIPS anywhere else — which is how a whole class of recording
 *        bugs went untested in CI. The VAD accepts this signal as speech, so a
 *        test can drive the real chunker, worker and actor with no platform
 *        dependency. It will not transcribe (nothing real does), which is fine:
 *        the engine in these tests is a script.
 * WHERE: session/e2e_tests.rs, for the tests that must run on every host.
 */
pub fn synthetic_voice(ms: u64, f0: f32) -> Vec<f32> {
    use std::f32::consts::TAU;

    let count = (ms * 16) as usize;
    let mut samples: Vec<f32> = (0..count)
        .map(|i| {
            let t = i as f32 / 16_000.0;
            let envelope = 0.5 + 0.5 * (TAU * 4.0 * t).sin().abs();
            let voiced: f32 = (1..16)
                .map(|harmonic| {
                    let frequency = f0 * harmonic as f32;
                    let gain = (-((frequency - 700.0) / 300.0).powi(2)).exp()
                        + 0.7 * (-((frequency - 1200.0) / 400.0).powi(2)).exp();
                    gain * (TAU * frequency * t).sin() / harmonic as f32
                })
                .sum();
            envelope * voiced
        })
        .collect();

    let peak = samples.iter().fold(1e-6_f32, |peak, sample| peak.max(sample.abs()));
    for sample in &mut samples {
        *sample = *sample / peak * 0.6;
    }
    samples
}

/**
 * WHAT:  Reads 32-bit float samples from a WAV file by locating its data chunk.
 * WHY:   See the module WHY — a fixed header skip is wrong for the files we
 *        actually generate.
 */
pub fn read_wav_f32(path: &Path) -> Option<Vec<f32>> {
    let bytes = std::fs::read(path).ok()?;
    // 12 bytes of RIFF/WAVE preamble, then a sequence of chunks.
    let mut pos = 12usize;

    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().ok()?) as usize;
        let body = pos + 8;

        if id == b"data" {
            let end = body.saturating_add(size).min(bytes.len());
            return Some(
                bytes[body..end]
                    // `as_chunks` rather than `chunks_exact(4)`: a constant
                    // chunk size gives a fixed-size array, so the four indexes
                    // below are checked at compile time instead of at runtime.
                    // Newer clippy denies the old form for exactly that reason,
                    // which is how CI caught this on a toolchain ahead of the
                    // one on this machine.
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| f32::from_le_bytes(*c))
                    .collect(),
            );
        }
        // Chunks are word-aligned, so an odd size carries a pad byte.
        pos = body.saturating_add(size) + (size & 1);
    }
    None
}

/**
 * WHAT:  Synthesises a sentence with macOS `say`, at exactly the format the
 *        pipeline delivers, and returns the samples.
 * WHY:   No committed audio fixtures. The file is cached in the temp directory
 *        so repeated runs do not re-synthesise, and generating it in the
 *        pipeline's own format means the fixture cannot drift from reality.
 *        Returns None rather than failing when `say` is unavailable, so the
 *        caller can skip instead of going red for an environmental reason.
 */
pub fn synthesise_speech(sentence: &str, name: &str) -> Option<Vec<f32>> {
    synthesise_speech_with_voice(sentence, name, None)
}

/**
 * WHAT:  As above, but with a specific system voice.
 * WHY:   The multilingual claim can only be tested against audio that is
 *        actually in that language, and `say` ships voices for them. Returns
 *        None when the voice is not installed, so a machine without it skips
 *        rather than failing for a reason that has nothing to do with the code.
 * WHERE: The Arabic detection test.
 */
pub fn synthesise_speech_with_voice(
    sentence: &str,
    name: &str,
    voice: Option<&str>,
) -> Option<Vec<f32>> {
    let path: PathBuf = std::env::temp_dir().join(format!("vocs-fixture-{name}.wav"));

    if !path.exists() {
        let mut command = std::process::Command::new("say");
        if let Some(voice) = voice {
            command.args(["-v", voice]);
        }
        let status = command
            .args([
                "-o",
                path.to_str()?,
                "--data-format=LEF32@16000",
                "--channels=1",
                "-r",
                "180",
                sentence,
            ])
            .status()
            .ok()?;
        if !status.success() {
            return None;
        }
    }

    let samples = read_wav_f32(&path)?;
    // A fixture that is silent or out of range would produce a confusing
    // failure downstream, so it is rejected here where the cause is obvious.
    if samples.is_empty() || samples.iter().any(|s| !s.is_finite() || s.abs() > 1.0) {
        return None;
    }
    Some(samples)
}

/**
 * SOURCE OF TRUTH KEYWORDS: shared_engine, TEST_ENGINE, ENGINE_LOCK,
 *   live_model_path, with_engine
 * WHAT:  ONE Whisper engine shared by every test in the binary, plus the lock
 *        that serialises access to it.
 * WHY:   Two separate reasons, and the second one is not an optimisation:
 *
 *        1. **ggml aborts on a second context.** whisper.cpp's Metal backend
 *           asserts `[rsets->data count] == 0` when a context is destroyed
 *           while another holds device resources. A test binary that builds an
 *           engine per test therefore does not merely run slowly — it CRASHES
 *           the process partway through, taking unrelated tests with it and
 *           producing failures that point at innocent code.
 *           This is the same rule production follows: exactly one context for
 *           the process lifetime. The tests now obey it too.
 *
 *        2. **Concurrent decodes destroy timings.** Five engines decoding at
 *           once on one machine made a capability probe read 7.12x instead of
 *           34.8x, and a latency assertion failed against a number that
 *           measured its own siblings rather than the product.
 *
 *        Every test that touches the engine must hold ENGINE_LOCK for its whole
 *        body. parking_lot, so a failing assertion reports itself rather than
 *        poisoning the lock and burying the cause under lock failures.
 * WHERE: Used by adapters/whisper/live_tests.rs and session/e2e_tests.rs. There
 *        must be no other OnceLock holding an engine anywhere in the crate.
 */
/**
 * SOURCE OF TRUTH KEYWORDS: TEST_ENGINE, release_engine_at_exit, ggml_rsets
 * WHAT:  The engine slot, and the process-exit hook that empties it.
 * WHY:   ggml frees its Metal device from a function-local `static` in
 *        ggml-metal-device.cpp, so the free runs at PROCESS EXIT via the C++
 *        atexit chain — and the first thing it does is
 *        `GGML_ASSERT([rsets->data count] == 0)`, whose own comment reads
 *        "most likely you haven't deallocated all Metal resources before
 *        exiting". So the abort is not caused by tearing the context down. It
 *        is caused by NOT tearing it down.
 *
 *        That makes leaking the engine exactly the wrong remedy — it
 *        guarantees the assert — and it makes a plain `static` one too, since
 *        Rust never drops statics. Both leave the context alive at exit, which
 *        is the one state ggml refuses to exit from.
 *
 *        Hence a slot that CAN be emptied, plus an `atexit` hook that empties
 *        it. Ordering is what makes this correct rather than hopeful: the C
 *        and C++ exit chains both run in reverse registration order, and
 *        ggml's device static is constructed during the model load — which
 *        necessarily happens before we register here. Our hook therefore runs
 *        BEFORE ggml's, frees the context and its states while the device is
 *        still alive, and leaves the residency set empty for the assert.
 * WHERE: shared_engine below. The app does not need this: it holds one context
 *        for the process lifetime and its quit path was verified clean.
 */
static TEST_ENGINE: Mutex<Option<Option<Arc<dyn TranscriptionEngine>>>> = Mutex::new(None);
static ENGINE_LOCK: Mutex<()> = Mutex::new(());

extern "C" {
    /// The C runtime's own atexit. Declared rather than pulled in as a
    /// dependency: it is one symbol, always linked, and adding a crate to the
    /// production dependency list for a test-only teardown is a worse trade.
    fn atexit(callback: extern "C" fn()) -> std::ffi::c_int;
}

/// Frees the shared engine while ggml's Metal device is still alive. Runs on
/// the C exit chain — see the WHY above for why the ordering holds.
extern "C" fn release_engine_at_exit() {
    // A test still holding a clone would keep the context alive and the assert
    // would fire anyway; by the time the exit chain runs, none can be.
    let taken = TEST_ENGINE.lock().take();
    drop(taken);
}

/// The default model, if it has been downloaded. `None` means skip.
pub fn live_model_path() -> Option<PathBuf> {
    let paths = AppPaths::resolve().ok()?;
    let path = paths.model_file("large-v3-turbo-q5_0");
    path.is_file().then_some(path)
}

/**
 * WHAT:  The shared, prepared engine, or None when no model is installed.
 * WHY:   Prepared once. `prepare()` costs ~1.5s and loads 574MB, so doing it
 *        per test is the difference between a suite that runs and one nobody
 *        waits for.
 */
pub fn shared_engine() -> Option<Arc<dyn TranscriptionEngine>> {
    let mut slot = TEST_ENGINE.lock();

    // Outer None means "not attempted"; inner None means "attempted, no model".
    // Two levels so a missing model is remembered rather than retried per test.
    if slot.is_none() {
        let built = build_shared_engine();
        if built.is_some() {
            // Registered only once, and only after the model load has already
            // constructed ggml's device static — which is what puts our hook
            // ahead of theirs on the exit chain.
            // SAFETY: `atexit` takes a plain `extern "C" fn` with no arguments
            // and no captured state, which is exactly what is passed.
            unsafe {
                atexit(release_engine_at_exit);
            }
        }
        *slot = Some(built);
    }

    slot.as_ref().and_then(Clone::clone)
}

fn build_shared_engine() -> Option<Arc<dyn TranscriptionEngine>> {
    let path = live_model_path()?;
    let engine = crate::adapters::build_engine(&crate::adapters::default_engine_id(), path).ok()?;
    engine.prepare().ok()?;
    Some(engine)
}

/// Serialises engine access. Hold this for the whole body of any test that
/// decodes — see the module WHY.
pub fn engine_lock() -> MutexGuard<'static, ()> {
    ENGINE_LOCK.lock()
}

/**
 * SOURCE OF TRUTH KEYWORDS: ScriptedGrammar, grammar_model_double
 * WHAT:  A GrammarModel that answers from a table, counts what it was asked,
 *        and can be told to be slow or to fail.
 * WHY:   The real model is a 121 MB download, so everything that does not
 *        depend on its quality — the budget, the cache, the guard's wiring,
 *        what a failure degrades to — is tested against this.
 */
pub struct ScriptedGrammar {
    answers: Vec<(String, String)>,
    pub calls: Mutex<Vec<String>>,
    /// What each call was allowed: (had a deadline, had a cancel flag).
    pub limits: Mutex<Vec<(bool, bool)>>,
    delay: std::time::Duration,
    fails: bool,
}

impl ScriptedGrammar {
    pub fn answering(answers: &[(&str, &str)]) -> Arc<Self> {
        Arc::new(Self {
            answers: answers.iter().map(|(a, b)| ((*a).to_string(), (*b).to_string())).collect(),
            calls: Mutex::new(Vec::new()),
            limits: Mutex::new(Vec::new()),
            delay: std::time::Duration::ZERO,
            fails: false,
        })
    }

    pub fn slow(answers: &[(&str, &str)], delay: std::time::Duration) -> Arc<Self> {
        let mut model = Arc::try_unwrap(Self::answering(answers)).unwrap_or_else(|_| unreachable!());
        model.delay = delay;
        Arc::new(model)
    }

    pub fn failing() -> Arc<Self> {
        let mut model = Arc::try_unwrap(Self::answering(&[])).unwrap_or_else(|_| unreachable!());
        model.fails = true;
        Arc::new(model)
    }
}

impl crate::ports::GrammarModel for ScriptedGrammar {
    fn id(&self) -> &'static str {
        "scripted-grammar"
    }
    fn install(&self, _: PathBuf) {}
    fn uninstall(&self) {}
    fn is_installed(&self) -> bool {
        true
    }
    fn is_loaded(&self) -> bool {
        true
    }
    fn warm(&self) -> crate::error::AppResult<()> {
        Ok(())
    }
    fn unload_if_idle(&self, _: std::time::Duration) -> bool {
        false
    }
    fn rewrite(
        &self,
        sentence: &str,
        limit: crate::ports::RewriteLimit<'_>,
    ) -> crate::error::AppResult<String> {
        self.limits.lock().push((limit.deadline.is_some(), limit.cancel.is_some()));
        // Honours the CANCEL flag while it "decodes", as the real model does, but
        // not the deadline: the older tests rely on a slow call that finishes.
        let started = std::time::Instant::now();
        while started.elapsed() < self.delay {
            if limit.cancel.is_some_and(|c| c.load(std::sync::atomic::Ordering::Relaxed)) {
                return Err(crate::error::AppError::new(crate::error::ErrorCode::FinalizeTimeout, "cancelled"));
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        self.calls.lock().push(sentence.to_string());
        if self.fails {
            return Err(crate::error::AppError::new(crate::error::ErrorCode::Internal, "scripted failure"));
        }
        Ok(self
            .answers
            .iter()
            .find(|(from, _)| from == sentence)
            .map_or_else(|| sentence.to_string(), |(_, to)| to.clone()))
    }
}
