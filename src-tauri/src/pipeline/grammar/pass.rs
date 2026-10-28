/*!
 * SOURCE OF TRUTH KEYWORDS: GrammarPass, PassOutcome, correct, prefetch,
 *   GRAMMAR_BUDGET, applies_to_language, protected_terms
 * WHAT:  Runs the on-device grammar model over a finished transcript, sentence
 *        by sentence, and returns the text with only the safe edits applied.
 * WHY:   Three things make this safe to leave on all day.
 *
 *        It is bounded in TIME. The model is ~100 ms a sentence, and a person
 *        who just stopped talking is waiting; past `GRAMMAR_BUDGET` the
 *        remaining sentences are delivered as the rules left them rather than
 *        held back. A correction that arrives late is worth less than a paste
 *        that arrives on time.
 *
 *        It is bounded in WHAT IT MAY CHANGE. Every rewrite goes through the
 *        edit guard (guard.rs); nothing the model says reaches the document
 *        unjudged.
 *
 *        It is mostly done already. `prefetch` rewrites the sentences that are
 *        finished while the person is still speaking and stores them in the
 *        cache, so at the stop only the last sentence needs the model — one
 *        ~100 ms call instead of N.
 *
 *        English only: the model was trained on English, and "fixing" Hindi
 *        with it would be a rewrite of a language it cannot read. An unknown
 *        language is treated as not English.
 * WHERE: Owned by SessionContext; called by session/delivery.rs (correct), by
 *        session/actor.rs while recording (prefetch), and by bootstrap tasks
 *        (warm and idle unload, through the model handle).
 */

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::ports::grammar::{GrammarModel, RewriteLimit};
use crate::types::{CorrectionEdit, DictionaryEntry, LanguageCode};

use super::cache::RewriteCache;
use super::guard::merge_safe_edits;
use super::sentences::split_sentences;

/// How long the stop-to-paste path may spend asking the model. Sentences not
/// reached are left as the rules wrote them. Enforced INSIDE each model call
/// (RewriteLimit), not only between sentences: it was 900 ms and could still be
/// overrun by a whole sentence, a held lock or a cold load, which is how a paste
/// took seconds with the on-device model on.
pub const GRAMMAR_BUDGET: Duration = Duration::from_millis(500);

/**
 * SOURCE OF TRUTH KEYWORDS: PassOutcome
 * WHAT:  The corrected text and what it cost.
 * WHY:   The counts are logged on every run: "cache_hits" proves the prefetch is
 *        earning its keep, and "left_alone" is the budget (or a model error)
 *        biting — the first thing to look at when someone says the grammar fix
 *        "sometimes doesn't happen".
 * WHERE: Returned by GrammarPass::correct.
 */
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassOutcome {
    pub text: String,
    pub sentences: usize,
    pub edits_kept: usize,
    pub edits_refused: usize,
    pub model_calls: usize,
    pub cache_hits: usize,
    pub left_alone: usize,
    /// The model's accepted fixes, as phrases that can be remembered.
    pub edits: Vec<CorrectionEdit>,
}

/**
 * SOURCE OF TRUTH KEYWORDS: PassOptions
 * WHAT:  What a caller may vary: the time it can spend, and whether the text is
 *        in the Formal writing style.
 * WHY:   A struct so a third knob does not become a fourth positional argument;
 *        `formal` is not cosmetic — see guard::merge_safe_edits.
 * WHERE: Built by session/delivery.rs.
 */
#[derive(Debug, Clone, Copy)]
pub struct PassOptions {
    pub budget: Duration,
    pub formal: bool,
}

impl Default for PassOptions {
    fn default() -> Self {
        Self { budget: GRAMMAR_BUDGET, formal: false }
    }
}

pub struct GrammarPass {
    model: Arc<dyn GrammarModel>,
    cache: RewriteCache,
    prefetching: AtomicBool,
    /// Set while the stop-time pass is running so a live prefetch lets go of the
    /// model. See `yield_to_final_pass`.
    stop_prefetch: AtomicBool,
}

/**
 * SOURCE OF TRUTH KEYWORDS: PrefetchYield, yield_to_final_pass
 * WHAT:  While alive, tells any running prefetch to stop and any new one to
 *        stand down; dropping it lets prefetching resume.
 * WHY:   The model is single-threaded behind one lock. A prefetch that is
 *        mid-sentence when the person stops talking would otherwise make the
 *        paste wait for it, and the paste is the one with a person attached.
 * WHERE: Held by GrammarPass::correct for the length of the pass.
 */
pub struct PrefetchYield<'a>(&'a AtomicBool);

impl Drop for PrefetchYield<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Words the person's dictionary owns, which the guard must never alter.
pub fn protected_terms(dictionary: &[DictionaryEntry]) -> HashSet<String> {
    dictionary
        .iter()
        .filter(|entry| entry.enabled)
        .flat_map(|entry| [entry.pattern.as_str(), entry.replacement.as_str()])
        .flat_map(str::split_whitespace)
        .map(|word| {
            word.trim_matches(|c: char| !c.is_alphanumeric())
                .replace('\u{2019}', "'")
                .to_lowercase()
        })
        .filter(|word| !word.is_empty())
        .collect()
}

impl GrammarPass {
    pub fn new(model: Arc<dyn GrammarModel>) -> Self {
        Self {
            model,
            cache: RewriteCache::default(),
            prefetching: AtomicBool::new(false),
            stop_prefetch: AtomicBool::new(false),
        }
    }

    /// See `PrefetchYield`.
    pub fn yield_to_final_pass(&self) -> PrefetchYield<'_> {
        self.stop_prefetch.store(true, Ordering::Release);
        PrefetchYield(&self.stop_prefetch)
    }

    /// The model handle, for the bootstrap tasks that install, warm and unload it.
    pub fn model(&self) -> &Arc<dyn GrammarModel> {
        &self.model
    }

    /// True when a verified model file is registered.
    pub fn is_available(&self) -> bool {
        self.model.is_installed()
    }

    /// The model reads English; anything else, including "unknown", is skipped.
    pub fn applies_to(language: Option<&LanguageCode>) -> bool {
        language.is_some_and(|l| l.as_str() == "en" || l.as_str().starts_with("en-"))
    }

    /// Forgets stored rewrites, e.g. after the model file is replaced.
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    /**
     * WHAT:  `text` with the guarded model corrections applied. Blocking.
     * WHY:   A model error on one sentence leaves that sentence alone and the
     *        pass carries on: a broken model must degrade to "the rules' text",
     *        never to a failed paste.
     * WHERE: session/delivery.rs, on a blocking thread.
     */
    pub fn correct(
        &self,
        text: &str,
        dictionary: &[DictionaryEntry],
        options: PassOptions,
    ) -> PassOutcome {
        let PassOptions { budget, formal } = options;
        let deadline = Instant::now() + budget;
        let _prefetch_yields = self.yield_to_final_pass();
        // Every model call is bounded by the same deadline as the loop, so the
        // budget holds for the call in flight too — see ports::grammar::RewriteLimit.
        let limit = RewriteLimit { deadline: Some(deadline), cancel: None };
        let protected = protected_terms(dictionary);
        let ranges = split_sentences(text);

        let mut outcome = PassOutcome {
            text: String::with_capacity(text.len()),
            sentences: ranges.len(),
            edits_kept: 0,
            edits_refused: 0,
            model_calls: 0,
            cache_hits: 0,
            left_alone: 0,
            edits: Vec::new(),
        };

        let mut cursor = 0;
        for range in ranges {
            outcome.text.push_str(&text[cursor..range.start]);
            cursor = range.end;
            let sentence = &text[range];

            let candidate = match self.cache.get(sentence) {
                Some(hit) => {
                    outcome.cache_hits += 1;
                    Some(hit)
                }
                None if Instant::now() >= deadline => None,
                None => {
                    outcome.model_calls += 1;
                    match self.model.rewrite(sentence, limit) {
                        Ok(rewrite) => {
                            self.cache.put(sentence, rewrite.clone());
                            Some(rewrite)
                        }
                        Err(err) => {
                            tracing::warn!(error = %err, "grammar model failed on a sentence; leaving it as written");
                            None
                        }
                    }
                }
            };

            match candidate {
                Some(rewrite) => {
                    let merged = merge_safe_edits(sentence, &rewrite, &protected, formal);
                    outcome.edits_kept += merged.accepted;
                    outcome.edits_refused += merged.rejected;
                    outcome.edits.extend(merged.edits);
                    outcome.text.push_str(&merged.text);
                }
                None => {
                    outcome.left_alone += 1;
                    outcome.text.push_str(sentence);
                }
            }
        }
        outcome.text.push_str(&text[cursor..]);
        outcome
    }

    /**
     * WHAT:  Rewrites the finished sentences of a transcript that is still
     *        being spoken, into the cache. Blocking; returns how many it ran.
     * WHY:   A sentence with no terminal punctuation is the one still being
     *        said, so it is skipped. If a previous prefetch is still running
     *        this returns at once: the next chunk's call will cover what it
     *        missed, and two would only fight over the model's lock with the
     *        final pass waiting behind them.
     * WHERE: session/actor.rs, on a blocking thread, after each decoded chunk.
     */
    pub fn prefetch(&self, partial_text: &str) -> usize {
        if self
            .prefetching
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return 0;
        }

        let mut ran = 0;
        for range in split_sentences(partial_text) {
            let sentence = &partial_text[range];
            let finished = sentence.ends_with(['.', '?', '!', '"', '\u{201d}']);
            if !finished || self.cache.contains(sentence) {
                continue;
            }
            // The stop-time pass outranks this: once it is running, give the
            // model back instead of finishing a sentence nobody is waiting for.
            if self.stop_prefetch.load(Ordering::Acquire) {
                break;
            }
            let limit = RewriteLimit { deadline: None, cancel: Some(&self.stop_prefetch) };
            match self.model.rewrite(sentence, limit) {
                Ok(rewrite) => {
                    self.cache.put(sentence, rewrite);
                    ran += 1;
                }
                Err(err) => {
                    tracing::debug!(error = %err, "grammar prefetch skipped a sentence");
                    break;
                }
            }
        }

        self.prefetching.store(false, Ordering::Release);
        ran
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::ScriptedGrammar;
    use crate::types::{DictionaryId, MatchKind};

    const FIXES: &[(&str, &str)] = &[
        ("She don't know.", "She doesn't know."),
        ("We was late.", "We were late."),
    ];

    #[test]
    fn each_sentence_is_corrected_and_the_gaps_between_them_survive() {
        let pass = GrammarPass::new(ScriptedGrammar::answering(FIXES));

        let out = pass.correct("She don't know.\n\nWe was late.", &[], PassOptions::default());

        assert_eq!(out.text, "She doesn't know.\n\nWe were late.");
        assert_eq!((out.sentences, out.model_calls, out.edits_kept), (2, 2, 2));
    }

    #[test]
    fn a_prefetched_sentence_costs_no_model_call_at_the_stop() {
        let model = ScriptedGrammar::answering(FIXES);
        let pass = GrammarPass::new(model.clone());

        // Spoken so far: one finished sentence and one still being said.
        assert_eq!(pass.prefetch("She don't know. We was"), 1);
        let out = pass.correct("She don't know. We was late.", &[], PassOptions::default());

        assert_eq!(out.text, "She doesn't know. We were late.");
        assert_eq!((out.cache_hits, out.model_calls), (1, 1));
        let calls = model.calls.lock().clone();
        assert_eq!(
            calls.iter().filter(|c| c.as_str() == "She don't know.").count(),
            1,
            "rewritten once, not twice"
        );
    }

    #[test]
    fn an_unfinished_sentence_is_not_prefetched() {
        let model = ScriptedGrammar::answering(FIXES);
        let pass = GrammarPass::new(model.clone());
        assert_eq!(pass.prefetch("She don't know"), 0);
        assert!(model.calls.lock().is_empty());
    }

    #[test]
    fn the_budget_leaves_the_remaining_sentences_as_the_rules_wrote_them() {
        let pass = GrammarPass::new(ScriptedGrammar::slow(FIXES, Duration::from_millis(30)));
        let options = PassOptions { budget: Duration::from_millis(10), formal: false };

        let out = pass.correct("She don't know. We was late.", &[], options);

        // The first call starts inside the budget; the second does not.
        assert_eq!(out.text, "She doesn't know. We was late.");
        assert_eq!((out.model_calls, out.left_alone), (1, 1));
    }

    #[test]
    fn the_stop_time_pass_bounds_every_model_call_with_its_deadline() {
        let model = ScriptedGrammar::answering(FIXES);
        let pass = GrammarPass::new(model.clone());

        pass.correct("She don't know. We was late.", &[], PassOptions::default());

        // (deadline, cancel): the paste is bounded by time, never by a flag.
        assert_eq!(*model.limits.lock(), vec![(true, false), (true, false)]);
    }

    #[test]
    fn a_background_prefetch_has_no_deadline_but_can_be_told_to_stop() {
        let model = ScriptedGrammar::answering(FIXES);
        let pass = GrammarPass::new(model.clone());

        pass.prefetch("She don't know.");

        assert_eq!(*model.limits.lock(), vec![(false, true)]);
    }

    #[test]
    fn a_running_prefetch_gives_the_model_up_when_the_final_pass_wants_it() {
        let model = ScriptedGrammar::slow(FIXES, Duration::from_secs(5));
        let pass = Arc::new(GrammarPass::new(model));

        let background = {
            let pass = Arc::clone(&pass);
            std::thread::spawn(move || pass.prefetch("She don't know. We was late."))
        };
        // Let the prefetch get into its first (slow) sentence.
        std::thread::sleep(Duration::from_millis(60));

        let started = Instant::now();
        let hold = pass.yield_to_final_pass();
        let ran = background.join().expect("prefetch thread");
        drop(hold);

        assert_eq!(ran, 0, "the interrupted sentence was not cached");
        assert!(started.elapsed() < Duration::from_secs(2), "it did not sit out the 5 s sentence");
    }

    #[test]
    fn prefetching_resumes_once_the_final_pass_is_done() {
        let model = ScriptedGrammar::answering(FIXES);
        let pass = GrammarPass::new(model);

        drop(pass.yield_to_final_pass());

        assert_eq!(pass.prefetch("She don't know."), 1);
    }

    #[test]
    fn a_failing_model_degrades_to_the_text_it_was_given() {
        let pass = GrammarPass::new(ScriptedGrammar::failing());

        let out = pass.correct("She don't know.", &[], PassOptions::default());

        assert_eq!(out.text, "She don't know.");
        assert_eq!(out.left_alone, 1);
    }

    #[test]
    fn the_guard_still_judges_what_the_model_says() {
        let pass = GrammarPass::new(ScriptedGrammar::answering(&[(
            "Send it to Priya.",
            "Send it to Priyanka.",
        )]));
        let out = pass.correct("Send it to Priya.", &[], PassOptions::default());
        assert_eq!(out.text, "Send it to Priya.");
        assert_eq!(out.edits_refused, 1);
    }

    #[test]
    fn dictionary_words_are_protected_from_the_model() {
        let dictionary = vec![DictionaryEntry {
            id: DictionaryId(1),
            pattern: "cube control".into(),
            replacement: "kubectl".into(),
            match_kind: MatchKind::Word,
            enabled: true,
            used_at: None,
        }];
        let terms = protected_terms(&dictionary);
        assert!(terms.contains("kubectl") && terms.contains("cube") && terms.contains("control"));
    }

    #[test]
    fn only_english_is_corrected() {
        assert!(GrammarPass::applies_to(Some(&LanguageCode("en".into()))));
        assert!(GrammarPass::applies_to(Some(&LanguageCode("en-GB".into()))));
        assert!(!GrammarPass::applies_to(Some(&LanguageCode("hi".into()))));
        assert!(!GrammarPass::applies_to(None), "unknown is not assumed to be English");
    }

    #[test]
    fn text_with_nothing_to_correct_is_returned_verbatim() {
        let pass = GrammarPass::new(ScriptedGrammar::answering(&[]));
        let out = pass.correct("  Okay.  ", &[], PassOptions::default());
        assert_eq!(out.text, "  Okay.  ");
        assert_eq!(out.sentences, 0);
    }
}
