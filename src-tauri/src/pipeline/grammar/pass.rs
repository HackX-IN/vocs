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

use crate::ports::grammar::GrammarModel;
use crate::types::{DictionaryEntry, LanguageCode};

use super::cache::RewriteCache;
use super::guard::merge_safe_edits;
use super::sentences::split_sentences;

/// How long the stop-to-paste path may spend asking the model. Sentences not
/// reached are left as the rules wrote them.
pub const GRAMMAR_BUDGET: Duration = Duration::from_millis(900);

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
}

pub struct GrammarPass {
    model: Arc<dyn GrammarModel>,
    cache: RewriteCache,
    prefetching: AtomicBool,
}

/// Words the person's dictionary owns, which the guard must never alter.
fn protected_terms(dictionary: &[DictionaryEntry]) -> HashSet<String> {
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
        Self { model, cache: RewriteCache::default(), prefetching: AtomicBool::new(false) }
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
    pub fn correct(&self, text: &str, dictionary: &[DictionaryEntry], budget: Duration) -> PassOutcome {
        let deadline = Instant::now() + budget;
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
                    match self.model.rewrite(sentence) {
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
                    let merged = merge_safe_edits(sentence, &rewrite, &protected);
                    outcome.edits_kept += merged.accepted;
                    outcome.edits_refused += merged.rejected;
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
            match self.model.rewrite(sentence) {
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
    use crate::error::{AppError, AppResult, ErrorCode};
    use crate::types::{DictionaryId, MatchKind};
    use parking_lot::Mutex;
    use std::path::PathBuf;

    /// A model that answers from a table and counts its calls.
    struct Scripted {
        answers: Vec<(&'static str, &'static str)>,
        calls: Mutex<Vec<String>>,
        fail: bool,
        delay: Duration,
    }

    impl Scripted {
        fn new(answers: Vec<(&'static str, &'static str)>) -> Arc<Self> {
            Arc::new(Self { answers, calls: Mutex::new(vec![]), fail: false, delay: Duration::ZERO })
        }
    }

    impl GrammarModel for Scripted {
        fn id(&self) -> &'static str {
            "scripted"
        }
        fn install(&self, _: PathBuf) {}
        fn uninstall(&self) {}
        fn is_installed(&self) -> bool {
            true
        }
        fn is_loaded(&self) -> bool {
            true
        }
        fn warm(&self) -> AppResult<()> {
            Ok(())
        }
        fn unload_if_idle(&self, _: Duration) -> bool {
            false
        }
        fn rewrite(&self, sentence: &str) -> AppResult<String> {
            std::thread::sleep(self.delay);
            self.calls.lock().push(sentence.to_string());
            if self.fail {
                return Err(AppError::new(ErrorCode::Internal, "boom"));
            }
            Ok(self
                .answers
                .iter()
                .find(|(from, _)| *from == sentence)
                .map_or_else(|| sentence.to_string(), |(_, to)| (*to).to_string()))
        }
    }

    const FIXES: &[(&str, &str)] = &[
        ("She don't know.", "She doesn't know."),
        ("We was late.", "We were late."),
    ];

    #[test]
    fn each_sentence_is_corrected_and_the_gaps_between_them_survive() {
        let model = Scripted::new(FIXES.to_vec());
        let pass = GrammarPass::new(model);

        let out = pass.correct("She don't know.\n\nWe was late.", &[], GRAMMAR_BUDGET);

        assert_eq!(out.text, "She doesn't know.\n\nWe were late.");
        assert_eq!((out.sentences, out.model_calls, out.edits_kept), (2, 2, 2));
    }

    #[test]
    fn a_prefetched_sentence_costs_no_model_call_at_the_stop() {
        let model = Scripted::new(FIXES.to_vec());
        let pass = GrammarPass::new(model.clone());

        // Spoken so far: one finished sentence and one still being said.
        assert_eq!(pass.prefetch("She don't know. We was"), 1);
        let out = pass.correct("She don't know. We was late.", &[], GRAMMAR_BUDGET);

        assert_eq!(out.text, "She doesn't know. We were late.");
        assert_eq!((out.cache_hits, out.model_calls), (1, 1));
        let calls = model.calls.lock().clone();
        assert_eq!(calls.iter().filter(|c| c.as_str() == "She don't know.").count(), 1, "rewritten once, not twice");
    }

    #[test]
    fn an_unfinished_sentence_is_not_prefetched() {
        let model = Scripted::new(FIXES.to_vec());
        let pass = GrammarPass::new(model.clone());
        assert_eq!(pass.prefetch("She don't know"), 0);
        assert!(model.calls.lock().is_empty());
    }

    #[test]
    fn the_budget_leaves_the_remaining_sentences_as_the_rules_wrote_them() {
        let model = Arc::new(Scripted {
            answers: FIXES.to_vec(),
            calls: Mutex::new(vec![]),
            fail: false,
            delay: Duration::from_millis(30),
        });
        let pass = GrammarPass::new(model);

        let out = pass.correct("She don't know. We was late.", &[], Duration::from_millis(10));

        // The first call starts inside the budget; the second does not.
        assert_eq!(out.text, "She doesn't know. We was late.");
        assert_eq!((out.model_calls, out.left_alone), (1, 1));
    }

    #[test]
    fn a_failing_model_degrades_to_the_text_it_was_given() {
        let model = Arc::new(Scripted {
            answers: vec![],
            calls: Mutex::new(vec![]),
            fail: true,
            delay: Duration::ZERO,
        });
        let pass = GrammarPass::new(model);

        let out = pass.correct("She don't know.", &[], GRAMMAR_BUDGET);

        assert_eq!(out.text, "She don't know.");
        assert_eq!(out.left_alone, 1);
    }

    #[test]
    fn the_guard_still_judges_what_the_model_says() {
        let model = Scripted::new(vec![("Send it to Priya.", "Send it to Priyanka.")]);
        let pass = GrammarPass::new(model);
        let out = pass.correct("Send it to Priya.", &[], GRAMMAR_BUDGET);
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
        let pass = GrammarPass::new(Scripted::new(vec![]));
        let out = pass.correct("  Okay.  ", &[], GRAMMAR_BUDGET);
        assert_eq!(out.text, "  Okay.  ");
        assert_eq!(out.sentences, 0);
    }
}
