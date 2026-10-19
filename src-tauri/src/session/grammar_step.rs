/*!
 * SOURCE OF TRUTH KEYWORDS: grammar_step, warm_grammar, prefetch_grammar,
 *   correct_grammar, GrammarEligibility, ai_grammar
 * WHAT:  The session's three touch points with the on-device grammar model:
 *        warm it when a recording starts, prefetch finished sentences while the
 *        person talks, and correct the finished transcript before it is pasted.
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
use crate::pipeline::grammar::{GrammarPass, PassOptions};
use crate::session::enhance_context::enhance_context;
use crate::session::SessionSettings;
use crate::services;
use crate::types::{DictionaryEntry, LanguageCode, Tone};

/// Whether this session should use the model at all, before the language is known.
fn wanted(ctx: &SessionContext, settings: &SessionSettings) -> bool {
    settings.ai_grammar && settings.tone != Tone::Literal && ctx.grammar.is_available()
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
        let context = enhance_context(&settings, language, dictionary);
        let Ok(text) = ctx.ports.enhancer.enhance(&partial_raw, &context) else {
            return;
        };
        let ran = ctx.grammar.prefetch(&text);
        if ran > 0 {
            tracing::debug!(sentences = ran, "prefetched grammar corrections while recording");
        }
    });
}

/**
 * WHAT:  `text` with the model's safe corrections applied, or `text` unchanged
 *        when the model is not wanted, not installed, or the language is not
 *        English.
 * WHY:   Awaited by the delivery worker, which is one task for ALL recordings —
 *        the blocking pool keeps a slow sentence from stalling anything but
 *        this paste.
 */
pub async fn correct(
    ctx: &SessionContext,
    settings: &SessionSettings,
    language: Option<&LanguageCode>,
    dictionary: Vec<DictionaryEntry>,
    text: String,
) -> String {
    if !wanted(ctx, settings) || !reads(language) {
        return text;
    }
    let grammar = ctx.grammar.clone();
    let options = PassOptions {
        formal: settings.tone == Tone::Formal,
        ..PassOptions::default()
    };
    let original = text.clone();

    let outcome = tokio::task::spawn_blocking(move || grammar.correct(&text, &dictionary, options)).await;
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
            outcome.text
        }
        Err(err) => {
            tracing::warn!(error = %err, "the grammar pass did not finish; using the rules' text");
            original
        }
    }
}
