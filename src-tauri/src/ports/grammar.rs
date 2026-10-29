/*!
 * SOURCE OF TRUTH KEYWORDS: GrammarModel, rewrite, warm, unload_if_idle,
 *   install, on_device_grammar
 * WHAT:  The trait for a small on-device model that rewrites ONE sentence into
 *        its grammatically corrected form.
 * WHY:   A port of its own and not a second TextEnhancer, because the contracts
 *        differ in the way that matters: the rule enhancer is pure and
 *        sub-millisecond, this one is a loadable resource that takes a tenth of
 *        a second a sentence and may not be installed at all. Folding it into
 *        TextEnhancer would let a slow, stateful call masquerade as a fast pure
 *        one. The port only rewrites; deciding which of the model's edits are
 *        safe to keep is policy and lives in pipeline/grammar, so swapping the
 *        model never means re-deriving the safety rules.
 *
 *        Lifecycle is explicit — install a verified file, warm (load), unload
 *        — because the weights are ~300 MB once loaded and an idle app should
 *        give that back.
 * WHERE: Implemented by adapters/t5; driven by pipeline/grammar and the
 *        bootstrap tasks that warm and unload it.
 */

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use crate::error::AppResult;

/**
 * SOURCE OF TRUTH KEYWORDS: RewriteLimit, rewrite_deadline, rewrite_cancel,
 *   bounded_rewrite
 * WHAT:  How long one `rewrite` may take and how it can be told to stop: an
 *        optional wall-clock deadline and an optional cancel flag.
 * WHY:   The caller's time budget used to be checked only BETWEEN sentences, so
 *        it could not bound the things that actually blow it: waiting for the
 *        model's lock while the live prefetch holds it, loading the weights
 *        cold, and one long sentence mid-decode. A paste that is due now cannot
 *        wait on any of those, so the limit travels INTO the call and the
 *        adapter honours it at each of those three points. `cancel` is how the
 *        stop-time pass tells a prefetch that is still running to get out of the
 *        way. `Default` is "no limit", which is what a background prefetch
 *        wants.
 * WHERE: Built by pipeline/grammar/pass.rs; honoured by adapters/t5.
 */
#[derive(Clone, Copy, Default)]
pub struct RewriteLimit<'a> {
    /// Return an error rather than finish after this instant.
    pub deadline: Option<Instant>,
    /// Return an error as soon as this is set.
    pub cancel: Option<&'a AtomicBool>,
}

pub trait GrammarModel: Send + Sync {
    fn id(&self) -> &'static str;

    /// Registers a model file the ModelStore has already hash-verified. Does
    /// not load it: loading is the caller's choice of thread.
    fn install(&self, path: PathBuf);

    /// Forgets the file and frees the weights.
    fn uninstall(&self);

    /// True when a verified file is registered. Never blocks.
    fn is_installed(&self) -> bool;

    /// True when the weights are in memory. Never blocks, even while a
    /// rewrite is running.
    fn is_loaded(&self) -> bool;

    /// Loads the weights if installed and not yet loaded. Blocking (~0.2 s) and
    /// idempotent, so it is safe to call on every recording start.
    fn warm(&self) -> AppResult<()>;

    /// Frees the weights when nothing has used them for `idle`. Returns whether
    /// it did.
    fn unload_if_idle(&self, idle: Duration) -> bool;

    /// The model's own correction of one sentence. Blocking. The result is
    /// UNTRUSTED — it may have changed meaning — which is why the caller runs
    /// it through the edit guard instead of pasting it.
    ///
    /// With a `deadline` in `limit` the call never loads the weights (that is
    /// the caller's `warm`, off the critical path), never waits for the model
    /// past the deadline and abandons a decode that would overrun it. Each of
    /// those is an `Err`, and the caller leaves the sentence as it was.
    fn rewrite(&self, sentence: &str, limit: RewriteLimit<'_>) -> AppResult<String>;
}
