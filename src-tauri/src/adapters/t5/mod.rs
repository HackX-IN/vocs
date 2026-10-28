/*!
 * SOURCE OF TRUTH KEYWORDS: T5GrammarModel, T5_MODEL_ID, GrammarModel, candle,
 *   t5_small, AventIQ, idle_unload, jfleg
 * WHAT:  The GrammarModel port over AventIQ-AI/T5-small-grammar-correction
 *        (T5-small, 60M parameters) running on the CPU through candle.
 * WHY:   Lazily loaded and unloadable. The weights are ~300 MB once widened to
 *        F32, which is a lot to hold for an app that sits idle in the menu bar
 *        for hours, so the file is only REGISTERED at startup, loaded when a
 *        recording begins (a rewrite never pays the 0.2 s load) and dropped
 *        after a quiet spell.
 *
 *        Two atomics mirror the installed and loaded facts so a status query
 *        never waits on the mutex a rewrite is holding.
 *
 *        CPU, not Metal: for a 60M model decoding one token at a time, the cost
 *        is dominated by kernel launches, not arithmetic — Accelerate on the
 *        CPU wins. Whisper keeps the GPU to itself, which also means a grammar
 *        pass never contends with the next recording's transcription.
 *
 *        The tokenizer and config ship in the binary, not beside the weights:
 *        they are fixed by the model revision the catalog pins, so downloading
 *        them would only add files that can be missing or mismatched. The
 *        tokenizer is google-t5/t5-small's (Apache-2.0); the fine-tune's own
 *        spiece.model has the identical SHA-256.
 * WHERE: Built by adapters/mod.rs::build_grammar_model; used through the port by
 *        pipeline/grammar.
 */

mod decode;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::error::{AppError, AppResult, ErrorCode};
use crate::ports::grammar::{GrammarModel, RewriteLimit};
use decode::{load_weights, out_of_time, out_of_time_error, T5Weights};

pub const T5_MODEL_ID: &str = "t5-small-grammar";

struct Inner {
    path: Option<PathBuf>,
    weights: Option<T5Weights>,
    last_used: Instant,
}

pub struct T5GrammarModel {
    inner: Mutex<Inner>,
    installed: AtomicBool,
    loaded: AtomicBool,
}

impl T5GrammarModel {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner { path: None, weights: None, last_used: Instant::now() }),
            installed: AtomicBool::new(false),
            loaded: AtomicBool::new(false),
        }
    }

    fn not_installed() -> AppError {
        AppError::new(
            ErrorCode::ModelMissing,
            "The grammar model is not installed. Download it in Settings → Output.",
        )
    }
}

impl Default for T5GrammarModel {
    fn default() -> Self {
        Self::new()
    }
}

impl GrammarModel for T5GrammarModel {
    fn id(&self) -> &'static str {
        T5_MODEL_ID
    }

    fn install(&self, path: PathBuf) {
        let mut inner = self.inner.lock();
        // A different file invalidates whatever was loaded from the old one.
        if inner.path.as_ref() != Some(&path) {
            inner.weights = None;
            self.loaded.store(false, Ordering::Release);
        }
        inner.path = Some(path);
        self.installed.store(true, Ordering::Release);
    }

    fn uninstall(&self) {
        let mut inner = self.inner.lock();
        inner.path = None;
        inner.weights = None;
        self.installed.store(false, Ordering::Release);
        self.loaded.store(false, Ordering::Release);
    }

    fn is_installed(&self) -> bool {
        self.installed.load(Ordering::Acquire)
    }

    fn is_loaded(&self) -> bool {
        self.loaded.load(Ordering::Acquire)
    }

    fn warm(&self) -> AppResult<()> {
        let mut inner = self.inner.lock();
        inner.last_used = Instant::now();
        if inner.weights.is_some() {
            return Ok(());
        }
        let path = inner.path.clone().ok_or_else(Self::not_installed)?;

        let started = Instant::now();
        inner.weights = Some(load_weights(&path)?);
        self.loaded.store(true, Ordering::Release);
        tracing::info!(elapsed_ms = started.elapsed().as_millis() as u64, "grammar model loaded");
        Ok(())
    }

    fn unload_if_idle(&self, idle: Duration) -> bool {
        // try_lock: a rewrite in progress means it is not idle, and the sweep
        // must not queue behind it.
        let Some(mut inner) = self.inner.try_lock() else {
            return false;
        };
        if inner.weights.is_some() && inner.last_used.elapsed() >= idle {
            inner.weights = None;
            self.loaded.store(false, Ordering::Release);
            tracing::info!("grammar model unloaded after sitting idle");
            return true;
        }
        false
    }

    /**
     * WHAT:  One sentence through the model, honouring `limit` at the three
     *        places it can be overrun: the wait for the lock, the cold load, and
     *        the decode itself.
     * WHY:   A caller with a deadline is a paste somebody is waiting for. The
     *        lock is held by whichever rewrite got there first — usually the live
     *        prefetch — so an unbounded `lock()` made the budget meaningless;
     *        it waits only until the deadline. A cold load (~0.2 s, more on a
     *        busy disk) is refused outright for the same reason: the sentence
     *        goes out as the rules wrote it, and `warm` brings the weights in
     *        behind the next recording instead. Without a deadline — the
     *        background prefetch — it blocks and loads as before.
     */
    fn rewrite(&self, sentence: &str, limit: RewriteLimit<'_>) -> AppResult<String> {
        if out_of_time(&limit) {
            return Err(out_of_time_error());
        }

        let mut inner = match limit.deadline {
            Some(deadline) => self.inner.try_lock_until(deadline).ok_or_else(out_of_time_error)?,
            None => self.inner.lock(),
        };

        if inner.weights.is_none() {
            if limit.deadline.is_some() {
                return Err(AppError::new(
                    ErrorCode::EngineNotReady,
                    "The grammar model is not loaded yet.",
                ));
            }
            let path = inner.path.clone().ok_or_else(Self::not_installed)?;
            inner.weights = Some(load_weights(&path)?);
            self.loaded.store(true, Ordering::Release);
        }
        inner.last_used = Instant::now();

        let weights = inner.weights.as_mut().ok_or_else(Self::not_installed)?;
        let out = weights.rewrite(sentence, limit);
        inner.last_used = Instant::now();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_that_was_never_installed_says_so_rather_than_loading_nothing() {
        let model = T5GrammarModel::new();
        assert!(!model.is_installed());
        assert!(!model.is_loaded());
        assert_eq!(model.warm().err().map(|e| e.code), Some(ErrorCode::ModelMissing));
        assert_eq!(
            model.rewrite("I has a idea.", RewriteLimit::default()).err().map(|e| e.code),
            Some(ErrorCode::ModelMissing)
        );
    }

    #[test]
    fn installing_registers_the_file_without_loading_it() {
        let model = T5GrammarModel::new();
        model.install(PathBuf::from("/nonexistent/model.safetensors"));
        assert!(model.is_installed());
        assert!(!model.is_loaded(), "install must not pay the load");
        // Warming a path that is not there fails cleanly and stays unloaded.
        assert!(model.warm().is_err());
        assert!(!model.is_loaded());
    }

    #[test]
    fn a_rewrite_already_past_its_deadline_returns_at_once() {
        let model = T5GrammarModel::new();
        let limit = RewriteLimit { deadline: Some(Instant::now()), cancel: None };
        let err = model.rewrite("I has a idea.", limit).expect_err("out of time");
        assert_eq!(err.code, ErrorCode::FinalizeTimeout);
    }

    #[test]
    fn a_cancelled_rewrite_is_refused_before_it_waits_for_anything() {
        let model = T5GrammarModel::new();
        let cancel = AtomicBool::new(true);
        let limit = RewriteLimit { deadline: None, cancel: Some(&cancel) };
        assert_eq!(model.rewrite("I has a idea.", limit).err().map(|e| e.code), Some(ErrorCode::FinalizeTimeout));
    }

    #[test]
    fn a_deadline_never_pays_for_a_cold_load() {
        // Installed but not loaded: the critical path must not load ~0.2 s of
        // weights, so it declines and leaves the sentence to the rules.
        let model = T5GrammarModel::new();
        model.install(PathBuf::from("/nonexistent/model.safetensors"));
        let limit = RewriteLimit { deadline: Some(Instant::now() + Duration::from_secs(5)), cancel: None };
        let err = model.rewrite("I has a idea.", limit).expect_err("not loaded");
        assert_eq!(err.code, ErrorCode::EngineNotReady);
        assert!(!model.is_loaded());
    }

    #[test]
    fn a_deadline_does_not_wait_for_a_rewrite_that_holds_the_lock() {
        let model = T5GrammarModel::new();
        let _held = model.inner.lock();
        let started = Instant::now();
        let limit = RewriteLimit {
            deadline: Some(started + Duration::from_millis(30)),
            cancel: None,
        };
        // `try_lock_until` in the same thread cannot succeed, which is the point:
        // it must give up at the deadline instead of deadlocking.
        let err = model.rewrite("I has a idea.", limit).expect_err("lock is taken");
        assert_eq!(err.code, ErrorCode::FinalizeTimeout);
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn uninstalling_forgets_the_file() {
        let model = T5GrammarModel::new();
        model.install(PathBuf::from("/nonexistent/model.safetensors"));
        model.uninstall();
        assert!(!model.is_installed());
        assert!(!model.unload_if_idle(Duration::ZERO));
    }

    /**
     * WHAT:  The real model on a real sentence, plus its load/unload cycle.
     * WHY:   Self-skipping unless the model has been downloaded (Settings →
     *        Output), the same convention as the whisper end-to-end tests: CI
     *        does not fetch 121 MB, and a developer machine that has it should
     *        exercise the actual decode.
     * WHERE: `cargo test` on a machine where the grammar model is installed.
     */
    #[test]
    fn the_real_model_corrects_a_sentence_and_unloads_when_idle() {
        let Ok(paths) = crate::config::AppPaths::resolve() else {
            eprintln!("skipped: no application support directory on this host");
            return;
        };
        let file = paths.asset_file("t5-grammar.safetensors");
        if !file.is_file() {
            eprintln!("skipped: the grammar model is not downloaded on this machine");
            return;
        }

        let model = T5GrammarModel::new();
        model.install(file);
        model.warm().expect("loads");
        assert!(model.is_loaded());

        let started = Instant::now();
        let out = model
            .rewrite("She don't know what we was talking about.", RewriteLimit::default())
            .expect("rewrites");
        eprintln!("rewrite took {:?}: {out}", started.elapsed());
        assert_eq!(out, "She doesn't know what we were talking about.");

        assert!(!model.unload_if_idle(Duration::from_secs(3600)), "not idle yet");
        assert!(model.unload_if_idle(Duration::ZERO));
        assert!(!model.is_loaded());
        assert!(model.is_installed(), "unloading is not uninstalling");
    }
}
