/*!
 * SOURCE OF TRUTH KEYWORDS: grammar_pipeline, GrammarPass, merge_safe_edits,
 *   RewriteCache, split_sentences, on_device_grammar
 * WHAT:  Barrel for the on-device grammar correction: sentence splitting, the
 *        model-output edit guard, the rewrite cache, and the pass that ties them
 *        to a GrammarModel.
 * WHY:   Four files because each is a different kind of risk. The splitter and
 *        the guard are pure and table-tested (where quality is decided), the
 *        cache is state, and the pass is the orchestration with the time budget.
 *        Nothing here knows which model sits behind the port.
 * WHERE: Used by session/delivery.rs and session/actor.rs through GrammarPass.
 */

pub mod cache;
pub mod guard;
pub mod pass;
pub mod sentences;

pub use pass::{GrammarPass, PassOutcome, GRAMMAR_BUDGET};
