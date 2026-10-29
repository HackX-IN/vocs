/*!
 * SOURCE OF TRUTH KEYWORDS: grammar_step, warm_grammar, prefetch_grammar,
 *   correct_grammar, GrammarEligibility, ai_grammar
 * WHAT:  The session's touch points with the grammar layer: warm the on-device
 *        model when a recording starts, prefetch finished sentences while the
 *        person talks, and correct the finished transcript before it is pasted —
 *        first with the fixes the user has taught Vocs, then with the model.
 * WHY:   One module so "when does the model run" has one answer. All three ask
 *        the same eligibility question first — the setting is on, the style is
 *        not Literal, the language is English, the model file is installed —
 *        and a copy of that test per caller is how one of them would end up
 *        running the model on a terminal command.
 *
 *        Every path is best effort and none can fail a dictation: a missing
 *        model, an error, or a blown time budget all end in "the rules' text",
 *        which is what the person would have got with the feature off.
 *
 *        The work is blocking CPU (~100 ms a sentence), so it runs on the
 *        blocking pool and never on the async worker that is also routing the
 *        next recording's events.
 * WHERE: warm and prefetch are called by session/actor.rs; correct by
 *        session/delivery.rs.
 */

use crate::ipc::context::SessionContext;
use crate::pipeline::grammar::learned::apply_learned;
use crate::pipeline::grammar::{protected_terms, GrammarPass, PassOptions};
use crate::session::enhance_context::enhance_context;
use crate::session::SessionSettings;
use crate::services;
use crate::types::{CorrectionEdit, DictionaryEntry, LanguageCode, Tone};

/// How much longer than the pass's own budget the delivery worker will wait for
/// it before giving up on the model for this paste. See `correct`.
const HARD_LIMIT_GRACE: std::time::Duration = std::time::Duration::from_millis(250);

/// Whether this session should use the model at all, before the language is known.
fn wanted(ctx: &SessionContext, settings: &SessionSettings) -> bool {
    settings.ai_grammar && settings.tone != Tone::Literal && ctx.grammar.is_available()
}

/**
 * WHAT:  Applies the fixes the user has taught Vocs, returning the text and
 *        which fixes it applied.
 * WHY:   Runs before the model and for the prefetch too, so the cache is keyed
 *        by the text the model will really be asked about. English only and
 *        never in the Literal style, for the same reasons as the model.
 */
fn apply_taught(
    ctx: &SessionContext,
    settings: &SessionSettings,
    language: Option<&LanguageCode>,
    dictionary: &[DictionaryEntry],
    text: String,
) -> (String, Vec<CorrectionEdit>) {
    if !settings.learn_corrections || settings.tone == Tone::Literal || !reads(language) {
        return (text, Vec::new());
    }
    let learned = services::learned::auto_applied(&ctx.db).unwrap_or_default();
    apply_learned(&text, &learned, &protected_terms(dictionary))
}

/// Whether the transcript's language is one the model reads.
fn reads(language: Option<&LanguageCode>) -> bool {
    GrammarPass::applies_to(language)
}

/**
 * WHAT:  Loads the model in the background so the first sentence of this
 *        recording does not pay for it. Fire and forget.
 * WHY:   Called at recording START, not at launch: the weights are only held
 *        in memory while the person is dictating (and for a few minutes
 *        after), and the recording itself is the cover for the 0.2 s load. An
 *        explicit non-English language skips it.
 */
pub fn warm(ctx: &SessionContext, settings: &SessionSettings) {
    if !wanted(ctx, settings) || ctx.grammar.model().is_loaded() {
        return;
    }
    if settings.language.as_ref().is_some_and(|l| !reads(Some(l))) {
        return;
    }
    let grammar = ctx.grammar.clone();
    tokio::task::spawn_blocking(move || {
        if let Err(err) = grammar.model().warm() {
            tracing::warn!(error = %err, "could not warm the grammar model");
        }
    });
}

/**
 * WHAT:  Rewrites the finished sentences of the transcript so far into the
 *        cache, off the actor's thread. Fire and forget.
 * WHY:   Runs the RULES on the partial transcript first, because the cache is
 *        keyed by the text the model will be asked about at the stop — and that
 *        is the rules' output (capitalised, punctuated, fillers gone), not
 *        Whisper's raw words. A near miss just costs the stop a model call.
 */
pub fn prefetch(
    ctx: &SessionContext,
    settings: &SessionSettings,
    language: Option<LanguageCode>,
    partial_raw: String,
) {
    let language = settings.language.clone().or(language);
    if !wanted(ctx, settings) || !reads(language.as_ref()) || partial_raw.trim().is_empty() {
        return;
    }
    let ctx = ctx.clone();
    let settings = settings.clone();
    tokio::task::spawn_blocking(move || {
        let dictionary = services::dictionary::enabled_entries(&ctx.db).unwrap_or_default();
        let context = enhance_context(&settings, language.clone(), dictionary.clone());
        let Ok(ruled) = ctx.ports.enhancer.enhance(&partial_raw, &context) else {
            return;
        };
        let (text, _) = apply_taught(&ctx, &settings, language.as_ref(), &dictionary, ruled);
        let ran = ctx.grammar.prefetch(&text);
        if ran > 0 {
            tracing::debug!(sentences = ran, "prefetched grammar corrections while recording");
        }
    });
}

/**
 * SOURCE OF TRUTH KEYWORDS: GrammarResult
 * WHAT:  The corrected text and the fixes that produced it.
 * WHY:   The fixes travel with the text so the delivery can store them on the
 *        session: that record is what the user's keep-or-undo is later counted
 *        against.
 * WHERE: Returned by `correct`; consumed by session/delivery.rs.
 */
pub struct GrammarResult {
    pub text: String,
    pub edits: Vec<CorrectionEdit>,
}

/**
 * WHAT:  `text` with the taught fixes and then the model's safe corrections
 *        applied, or `text` unchanged when neither applies.
 * WHY:   Awaited by the delivery worker, which is one task for ALL recordings —
 *        the blocking pool keeps a slow sentence from stalling anything but
 *        this paste. The taught fixes run first and cost microseconds; the model
 *        then sees text they have already fixed, so it cannot "propose" the same
 *        fix again and double-count it.
 */
pub async fn correct(
    ctx: &SessionContext,
    settings: &SessionSettings,
    language: Option<&LanguageCode>,
    dictionary: Vec<DictionaryEntry>,
    text: String,
) -> GrammarResult {
    let (text, mut edits) = apply_taught(ctx, settings, language, &dictionary, text);

    if !wanted(ctx, settings) || !reads(language) {
        return GrammarResult { text, edits };
    }
    // A bounded pass never loads the weights itself (they would blow its budget),
    // so a model that was unloaded since the recording began is brought back in
    // the background for the NEXT dictation rather than left cold.
    warm(ctx, settings);
    let grammar = ctx.grammar.clone();
    let options = PassOptions { formal: settings.tone == Tone::Formal, ..PassOptions::default() };
    let original = text.clone();

    /*
     * SOURCE OF TRUTH KEYWORDS: grammar_hard_deadline, paste_never_waits_on_the_model
     * The pass bounds its own model calls (RewriteLimit), so this should never
     * fire. It is here because the delivery worker is ONE task for every
     * recording: anything that wedges inside this await — a saturated blocking
     * pool, a model call that stops honouring its deadline — would hold every
     * later paste behind it. The abandoned blocking task finishes by itself, since
     * its deadline has passed; the person gets the rules' text now.
     */
    let hard_limit = options.budget + HARD_LIMIT_GRACE;
    let outcome = match tokio::time::timeout(
        hard_limit,
        tokio::task::spawn_blocking(move || grammar.correct(&text, &dictionary, options)),
    )
    .await
    {
        Ok(joined) => joined,
        Err(_elapsed) => {
            tracing::warn!(limit_ms = hard_limit.as_millis() as u64, "the grammar pass overran its hard limit; using the rules' text");
            return GrammarResult { text: original, edits };
        }
    };
    match outcome {
        Ok(outcome) => {
            tracing::info!(
                sentences = outcome.sentences,
                model_calls = outcome.model_calls,
                cache_hits = outcome.cache_hits,
                kept = outcome.edits_kept,
                refused = outcome.edits_refused,
                left_alone = outcome.left_alone,
                "grammar pass"
            );
            if settings.learn_corrections {
                edits.extend(outcome.edits);
            }
            GrammarResult { text: outcome.text, edits }
        }
        Err(err) => {
            tracing::warn!(error = %err, "the grammar pass did not finish; using the rules' text");
            GrammarResult { text: original, edits }
        }
    }
}
