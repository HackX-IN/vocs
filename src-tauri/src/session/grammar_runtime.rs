/*!
 * SOURCE OF TRUTH KEYWORDS: grammar_runtime, run_grammar_housekeeping,
 *   adopt_grammar_model, IDLE_UNLOAD_AFTER, grammar_install
 * WHAT:  Keeps the grammar model's lifecycle in step with the files on disk:
 *        registers the verified model at startup and frees its memory after it
 *        has sat idle.
 * WHY:   Separate from grammar_step because it is not about a recording — it is
 *        about whether the model exists and whether it is worth 300 MB of RAM
 *        right now. The install goes through the ModelStore so the file that
 *        reaches the model has passed the same SHA-256 check as the whisper
 *        weights; a model the user dropped into the folder by hand does not.
 *
 *        A file that exists but has not been hashed yet is VERIFIED, not
 *        ensured: `ensure` would re-download a 121 MB file that failed its hash
 *        without the user asking, in an app that promises its only network
 *        traffic after setup is the update check.
 * WHERE: run_grammar_housekeeping is spawned by bootstrap; adopt/release are
 *        called by ipc/commands/models.rs after a download or a delete.
 */

use std::sync::Arc;
use std::time::Duration;

use crate::pipeline::grammar::GrammarPass;
use crate::ports::ModelStore;
use crate::types::{ModelKind, ModelState};

/// Long enough that dictating a few times an hour never pays the 0.2 s reload,
/// short enough that an idle laptop gives the memory back.
pub const IDLE_UNLOAD_AFTER: Duration = Duration::from_secs(5 * 60);
const SWEEP_EVERY: Duration = Duration::from_secs(60);

/// Registers a freshly verified model file and forgets rewrites made with any
/// previous one.
pub fn adopt_grammar_model(grammar: &GrammarPass, path: std::path::PathBuf) {
    grammar.model().install(path);
    grammar.clear_cache();
}

/// The model file was removed.
pub fn release_grammar_model(grammar: &GrammarPass) {
    grammar.model().uninstall();
    grammar.clear_cache();
}

/// Installs the model if it is on disk and verified. Safe to call repeatedly.
async fn install_if_present(grammar: &GrammarPass, models: &dyn ModelStore) {
    let Ok(listed) = models.list().await else {
        return;
    };
    let Some(entry) = listed.into_iter().find(|m| m.descriptor.kind == ModelKind::Grammar) else {
        return;
    };

    let ready = match entry.state {
        ModelState::Ready => entry.path,
        // On disk, right size, never hashed in this install: hash it now.
        ModelState::Verifying => match models.verify(&entry.descriptor.id).await {
            Ok(true) => models.status(&entry.descriptor.id).await.ok().and_then(|s| s.path),
            _ => None,
        },
        _ => None,
    };
    if let Some(path) = ready {
        adopt_grammar_model(grammar, path);
        tracing::info!("grammar model registered");
    }
}

/// Runs for the life of the app: install once, then sweep for idleness.
pub async fn run_grammar_housekeeping(grammar: Arc<GrammarPass>, models: Arc<dyn ModelStore>) {
    install_if_present(&grammar, models.as_ref()).await;

    let mut ticker = tokio::time::interval(SWEEP_EVERY);
    loop {
        ticker.tick().await;
        let model = Arc::clone(grammar.model());
        // try_lock inside, so this never queues behind a rewrite — but the call
        // is still synchronous, hence the blocking pool.
        let _ = tokio::task::spawn_blocking(move || model.unload_if_idle(IDLE_UNLOAD_AFTER)).await;
    }
}
