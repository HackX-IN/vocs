/*!
 * SOURCE OF TRUTH KEYWORDS: apply_learned, learned_phrase_match, auto_apply,
 *   personal_mistake_dictionary, CorrectionEdit
 * WHAT:  Applies the corrections the user has taught Vocs, without asking the
 *        model, and reports which ones it applied.
 * WHY:   A fix the user has kept three times and never undone is a fact about
 *        THEM, and it should not cost a model call — or need the model at all —
 *        to repeat. It is matched on whole words, case-blind, as an exact phrase:
 *        "he go" in "oh, he go" matches, "the go" does not, and punctuation
 *        between the words breaks the match so a comma-separated list is never
 *        stitched into a phrase it was not. Capitalisation of the first word and
 *        the punctuation at both ends are kept as spoken. Words the user's
 *        dictionary owns are never rewritten, even by a fix learned before the
 *        term was added.
 *
 *        It reports what it applied so the session can record it: if the user
 *        then chooses the original wording, that fix is counted AGAINST — which
 *        is the only way an auto-applied fix can ever stop.
 * WHERE: Called by session/grammar_step.rs before the model runs.
 */

use std::collections::HashSet;

use super::tokens::{tokenise, Tok};
use crate::types::{CorrectionEdit, LearnedCorrection};

/// Replaces `text`'s matches of each learned phrase, longest pattern first.
pub fn apply_learned(
    text: &str,
    learned: &[LearnedCorrection],
    protected: &HashSet<String>,
) -> (String, Vec<CorrectionEdit>) {
    if learned.is_empty() || text.trim().is_empty() {
        return (text.to_string(), Vec::new());
    }

    // Longest first, so "is best" -> "is the best" is not pre-empted by a shorter
    // overlapping fix.
    let mut ordered: Vec<(Vec<&str>, &LearnedCorrection)> = learned
        .iter()
        .map(|c| (c.pattern.split_whitespace().collect::<Vec<_>>(), c))
        .filter(|(words, _)| !words.is_empty())
        .collect();
    ordered.sort_by_key(|(words, _)| std::cmp::Reverse(words.len()));

    let (toks, gaps) = tokenise(text);
    let mut out = String::with_capacity(text.len() + 16);
    let mut applied = Vec::new();
    let mut i = 0;

    while i < toks.len() {
        let hit = ordered.iter().find(|(words, _)| matches_at(&toks, i, words, protected));
        match hit {
            Some((words, correction)) => {
                let n = words.len();
                out.push_str(&gaps[i]);
                out.push_str(&rewritten(&toks[i], &toks[i + n - 1], &correction.replacement));
                applied.push(CorrectionEdit {
                    pattern: correction.pattern.clone(),
                    replacement: correction.replacement.clone(),
                    auto: true,
                });
                i += n;
            }
            None => {
                out.push_str(&gaps[i]);
                out.push_str(&toks[i].text());
                i += 1;
            }
        }
    }
    out.push_str(&gaps[toks.len()]);
    (out, applied)
}

/**
 * WHAT:  Whether `phrase` (lowercase words) appears in `text` as whole,
 *        unpunctuated-between words.
 * WHY:   The settle step uses it to ask "is the fix still in the text?", and it
 *        must answer with the same idea of a phrase that applying one uses.
 * WHERE: session/learning.rs.
 */
pub fn contains_phrase(text: &str, phrase: &str) -> bool {
    let words: Vec<&str> = phrase.split_whitespace().collect();
    if words.is_empty() {
        return false;
    }
    let (toks, _) = tokenise(text);
    let none = HashSet::new();
    (0..toks.len()).any(|at| matches_at(&toks, at, &words, &none))
}

/// Whether `words` is the phrase starting at token `at`.
fn matches_at(toks: &[Tok], at: usize, words: &[&str], protected: &HashSet<String>) -> bool {
    if at + words.len() > toks.len() {
        return false;
    }
    let span = &toks[at..at + words.len()];
    span.iter().zip(words).all(|(t, w)| t.norm == *w && !protected.contains(&t.norm))
        // Punctuation between two of the words means they are not a phrase.
        && span[..span.len() - 1].iter().all(|t| t.trail.is_empty())
        && span[1..].iter().all(|t| t.lead.is_empty())
}

fn rewritten(first: &Tok, last: &Tok, replacement: &str) -> String {
    let mut words = replacement.split_whitespace().map(str::to_string).collect::<Vec<_>>();
    if first.core.chars().next().is_some_and(char::is_uppercase) {
        if let Some(word) = words.first_mut() {
            let mut chars = word.chars();
            if let Some(initial) = chars.next() {
                *word = initial.to_uppercase().chain(chars).collect();
            }
        }
    }
    format!("{}{}{}", first.lead, words.join(" "), last.trail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LearnedId;

    fn fix(pattern: &str, replacement: &str) -> LearnedCorrection {
        LearnedCorrection {
            id: LearnedId(1),
            pattern: pattern.into(),
            replacement: replacement.into(),
            accepted: 3,
            rejected: 0,
            auto_apply: true,
            manual: false,
            updated_at: 0,
        }
    }

    fn apply(text: &str, fixes: &[LearnedCorrection]) -> (String, usize) {
        let (out, applied) = apply_learned(text, fixes, &HashSet::new());
        (out, applied.len())
    }

    #[test]
    fn a_learned_phrase_is_replaced_wherever_it_appears() {
        let fixes = [fix("he go", "he goes")];
        assert_eq!(apply("Then he go home, and he go again.", &fixes), ("Then he goes home, and he goes again.".into(), 2));
    }

    #[test]
    fn capitalisation_and_end_punctuation_are_kept() {
        let fixes = [fix("he go", "he goes")];
        assert_eq!(apply("He go.", &fixes).0, "He goes.");
        assert_eq!(apply("(he go) now", &fixes).0, "(he goes) now");
    }

    #[test]
    fn it_matches_whole_words_only() {
        let fixes = [fix("he go", "he goes")];
        assert_eq!(apply("The he gone home.", &fixes).1, 0);
        assert_eq!(apply("the go there", &fixes).1, 0);
    }

    #[test]
    fn punctuation_between_the_words_breaks_the_phrase() {
        let fixes = [fix("he go", "he goes")];
        assert_eq!(apply("he, go home", &fixes), ("he, go home".into(), 0));
    }

    #[test]
    fn the_longest_phrase_wins_and_matches_do_not_overlap() {
        let fixes = [fix("is best", "is the best"), fix("is", "was")];
        assert_eq!(apply("She is best here.", &fixes).0, "She is the best here.");
    }

    #[test]
    fn insertions_and_removals_work_as_phrases() {
        assert_eq!(apply("She is best player.", &[fix("is best", "is the best")]).0, "She is the best player.");
        assert_eq!(apply("I want to to go.", &[fix("to to", "to")]).0, "I want to go.");
    }

    #[test]
    fn a_dictionary_word_is_never_rewritten() {
        let fixes = [fix("kubectl get", "kubectl gets")];
        let protected: HashSet<String> = ["kubectl".to_string()].into();
        let (out, applied) = apply_learned("run kubectl get now", &fixes, &protected);
        assert_eq!((out.as_str(), applied.len()), ("run kubectl get now", 0));
    }

    #[test]
    fn a_phrase_is_found_only_as_whole_words() {
        assert!(contains_phrase("Then he goes home.", "he goes"));
        assert!(contains_phrase("HE GOES.", "he goes"));
        assert!(!contains_phrase("Then he goes, home.", "goes home"));
        assert!(!contains_phrase("the doesnt", "does"));
        assert!(!contains_phrase("anything", ""));
    }

    #[test]
    fn what_was_applied_is_reported_as_automatic() {
        let (_, applied) = apply_learned("he go", &[fix("he go", "he goes")], &HashSet::new());
        assert_eq!(applied.len(), 1);
        assert!(applied[0].auto);
    }

    #[test]
    fn whitespace_survives_and_nothing_learned_changes_nothing() {
        assert_eq!(apply("a  b\nc", &[]).0, "a  b\nc");
        // Spacing around a matched phrase survives; inside it, it is one space.
        assert_eq!(apply("a  he  go\nhome", &[fix("he go", "he goes")]).0, "a  he goes\nhome");
    }
}
