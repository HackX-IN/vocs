/*!
 * SOURCE OF TRUTH KEYWORDS: strip_fillers, is_hesitation, HESITATIONS,
 *   DISCOURSE_FILLERS, PARENTHETICAL_ONLY, protected_words, filler_removal
 * WHAT:  Removes spoken filler from an English transcript: hesitations ("um",
 *        "uh", "er") always, and discourse words ("like", "you know", "I mean")
 *        only where the punctuation says they are filler.
 * WHY:   The first version deleted every "like" it found, so "I like pizza"
 *        came out as "I pizza". A word that is sometimes filler and sometimes a
 *        verb, a preposition or a noun cannot be judged by its spelling, but
 *        Whisper is trained on punctuated text and sets a filler off with
 *        commas ("It was, like, really good" / "I went there, you know."), while
 *        a real use sits inside its clause with none. So:
 *          - hesitations are never real words in dictation: always removed;
 *          - "like", "you know", "I mean", "sort of", "kind of" go only when
 *            they start or end a clause, with a comma or stop on the far side;
 *          - "actually", "basically", "literally" carry meaning ("Actually, no"
 *            reverses a sentence), so they go only when BRACKETED by commas on
 *            both sides, never at the start of a sentence.
 *        The cost is a miss when Whisper left out the commas. That is the right
 *        direction to be wrong in: a stray "like" is noise, a deleted verb is a
 *        different sentence.
 *
 *        Works on whitespace tokens so punctuation can be repaired as words
 *        leave — a dropped "um," must not leave ", ," behind, and a dropped
 *        final "you know." must hand its full stop to the word before it. A
 *        token with leading punctuation (an opening quote or bracket) is never
 *        touched, which also keeps quoted speech intact.
 *
 *        Terms the user put in their dictionary are protected: a band called
 *        "Like", a product called "Uh", is theirs to keep.
 * WHERE: Called by adapters/rules/mod.rs ahead of the dictionary pass.
 */

use crate::types::{DictionaryEntry, LanguageCode};

/// Collapsed form (runs of one letter folded to one) of sounds with no meaning.
/// "ummm" → "um", "hmmm" → "hm", "errr" → "er".
const HESITATIONS: &[&str] = &["um", "uh", "uhm", "er", "erm", "hm", "mhm"];

/// Removed when they start or end a clause, with punctuation on the far side.
const DISCOURSE_FILLERS: &[&[&str]] = &[
    &["you", "know"],
    &["i", "mean"],
    &["sort", "of"],
    &["kind", "of"],
    &["like"],
];

/// Removed only when commas bracket them on both sides.
const PARENTHETICAL_ONLY: &[&[&str]] = &[&["actually"], &["basically"], &["literally"]];

const CLAUSE_PUNCTUATION: [char; 5] = [',', ';', ':', '-', '\u{2014}'];
const SENTENCE_END: [char; 3] = ['.', '!', '?'];

/// One whitespace-delimited token split into the word and its trailing marks.
struct Token<'a> {
    /// The word with trailing punctuation removed; empty for pure punctuation.
    core: &'a str,
    /// Trailing punctuation, e.g. "," or ".".
    tail: &'a str,
    /// True when the token begins with punctuation (a quote, a bracket, a dash).
    opens_with_mark: bool,
    /// True when the token sits inside, or opens or closes, a double-quoted run.
    /// Quoted speech is somebody's exact words and is never edited.
    quoted: bool,
}

fn tokenise(text: &str) -> Vec<Token<'_>> {
    let mut inside_quote = false;
    text.split_whitespace()
        .map(|raw| {
            let marks = raw.chars().filter(|c| matches!(c, '"' | '\u{201C}' | '\u{201D}')).count();
            let quoted = inside_quote || marks > 0;
            // An odd number of quote marks flips whether the next token is inside.
            if marks % 2 == 1 {
                inside_quote = !inside_quote;
            }
            let opens_with_mark = raw.chars().next().is_some_and(|c| !c.is_alphanumeric());
            let core_end = raw
                .char_indices()
                .rev()
                .find(|(_, c)| c.is_alphanumeric() || *c == '\'')
                .map_or(0, |(i, c)| i + c.len_utf8());
            Token {
                core: &raw[..core_end],
                tail: &raw[core_end..],
                opens_with_mark,
                quoted,
            }
        })
        .collect()
}

/// Folds a run of the same letter to one: "ummm" → "um".
fn collapse_repeats(word: &str) -> String {
    let mut out = String::with_capacity(word.len());
    for ch in word.chars().flat_map(char::to_lowercase) {
        if out.chars().next_back() != Some(ch) {
            out.push(ch);
        }
    }
    out
}

fn is_hesitation(core: &str) -> bool {
    !core.is_empty() && HESITATIONS.contains(&collapse_repeats(core).as_str())
}

/// Lower-cased words of every enabled dictionary pattern and replacement.
fn protected_words(dictionary: &[DictionaryEntry]) -> Vec<String> {
    dictionary
        .iter()
        .filter(|entry| entry.enabled)
        .flat_map(|entry| [entry.pattern.as_str(), entry.replacement.as_str()])
        .flat_map(str::split_whitespace)
        .map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
        .filter(|word| !word.is_empty())
        .collect()
}

fn is_english(language: Option<&LanguageCode>) -> bool {
    language.is_some_and(|l| l.as_str().starts_with("en"))
}

/// Whether the tokens starting at `at` spell out `phrase`, with no mark inside
/// the phrase (a comma between "you" and "know" means they are not one filler).
fn phrase_at(tokens: &[Token<'_>], at: usize, phrase: &[&str]) -> bool {
    let Some(span) = tokens.get(at..at + phrase.len()) else {
        return false;
    };
    span.iter().enumerate().all(|(i, token)| {
        let inner_ok = i + 1 == span.len() || token.tail.is_empty();
        !token.opens_with_mark
            && !token.quoted
            && inner_ok
            && token.core.eq_ignore_ascii_case(phrase[i])
    })
}

/// Which kind of boundary precedes token `at`.
fn left_boundary(tokens: &[Token<'_>], at: usize) -> Boundary {
    match at.checked_sub(1).and_then(|i| tokens.get(i)) {
        None => Boundary::Start,
        Some(prev) if prev.tail.contains(SENTENCE_END) => Boundary::Start,
        Some(prev) if prev.tail.contains(CLAUSE_PUNCTUATION) => Boundary::Comma,
        Some(_) => Boundary::None,
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Boundary {
    /// Start of the text or of a sentence.
    Start,
    /// After a comma or similar.
    Comma,
    None,
}

/// Whether the phrase is closed off on its right: ends the text, or carries a
/// comma / stop of its own. `comma_only` demands a comma (the bracketed rule).
fn closes_clause(tail: &str, is_last: bool, comma_only: bool) -> bool {
    if comma_only {
        return tail.starts_with(',');
    }
    is_last && tail.is_empty() || tail.starts_with(CLAUSE_PUNCTUATION) || tail.starts_with(SENTENCE_END)
}

/// Length in tokens of the discourse filler starting at `at`, if it is one.
fn discourse_filler_at(tokens: &[Token<'_>], at: usize, protected: &[String]) -> Option<usize> {
    let boundary = left_boundary(tokens, at);

    let candidates = DISCOURSE_FILLERS
        .iter()
        .map(|phrase| (*phrase, false))
        .chain(PARENTHETICAL_ONLY.iter().map(|phrase| (*phrase, true)));

    for (phrase, parenthetical) in candidates {
        if !phrase_at(tokens, at, phrase) {
            continue;
        }
        if phrase.iter().any(|word| protected.iter().any(|p| p == word)) {
            return None;
        }
        let last = at + phrase.len() - 1;
        let tail = tokens[last].tail;
        let left_ok = if parenthetical {
            boundary == Boundary::Comma
        } else {
            boundary != Boundary::None
        };
        if left_ok && closes_clause(tail, last + 1 == tokens.len(), parenthetical) {
            return Some(phrase.len());
        }
    }
    None
}

fn capitalise_first(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/**
 * WHAT:  The transcript with filler words removed and the punctuation around
 *        them repaired.
 * WHY:   See the module WHY. Language-gated: the English lists would delete real
 *        words in Hindi or Arabic ("er" and "like" are not filler there).
 * WHERE: adapters/rules/mod.rs, when the strip-fillers setting is on.
 */
pub fn strip_fillers(
    text: &str,
    language: Option<&LanguageCode>,
    dictionary: &[DictionaryEntry],
) -> String {
    if !is_english(language) {
        return text.to_string();
    }

    let protected = protected_words(dictionary);
    // Line by line: a newline (from "new line" / "new paragraph") must survive,
    // and it is also a sentence boundary the token walk should respect.
    text.split('\n')
        .map(|line| strip_line(line, &protected))
        .collect::<Vec<_>>()
        .join("\n")
}

fn strip_line(line: &str, protected: &[String]) -> String {
    let tokens = tokenise(line);
    let mut out: Vec<String> = Vec::with_capacity(tokens.len());
    // A dropped token that began a sentence passes its capital to the next word.
    let mut carry_capital = false;
    let mut at = 0;

    while at < tokens.len() {
        let token = &tokens[at];

        let dropped = if !token.opens_with_mark
            && !token.quoted
            && is_hesitation(token.core)
            && !protected.iter().any(|p| p.eq_ignore_ascii_case(token.core))
        {
            Some(1)
        } else {
            discourse_filler_at(&tokens, at, protected)
        };

        let Some(width) = dropped else {
            let word = text_of(&tokens[at]);
            out.push(if std::mem::take(&mut carry_capital) { capitalise_first(&word) } else { word });
            at += 1;
            continue;
        };

        let first = &tokens[at];
        let last = &tokens[at + width - 1];
        let starts_sentence = left_boundary(&tokens, at) == Boundary::Start;
        if starts_sentence && first.core.chars().next().is_some_and(char::is_uppercase) {
            carry_capital = true;
        }

        // A stop on the dropped phrase ends the sentence the word before it is
        // in. Commas are the filler's own debris and go with it.
        if let Some(end) = last.tail.chars().find(|c| SENTENCE_END.contains(c)) {
            if let Some(previous) = out.last_mut() {
                let trimmed = previous.trim_end_matches(CLAUSE_PUNCTUATION).len();
                previous.truncate(trimmed);
                if !previous.ends_with(SENTENCE_END) {
                    previous.push(end);
                }
            }
            carry_capital = true;
        }
        at += width;
    }

    out.join(" ")
}

fn text_of(token: &Token<'_>) -> String {
    format!("{}{}", token.core, token.tail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DictionaryId, MatchKind};

    fn en() -> LanguageCode {
        LanguageCode("en".into())
    }

    fn strip(text: &str) -> String {
        strip_fillers(text, Some(&en()), &[])
    }

    fn protect(pattern: &str, replacement: &str) -> DictionaryEntry {
        DictionaryEntry {
            id: DictionaryId(1),
            pattern: pattern.into(),
            replacement: replacement.into(),
            match_kind: MatchKind::Word,
            enabled: true,
            used_at: None,
        }
    }

    // ── the spec's acceptance case ───────────────────────────────────────

    #[test]
    fn hesitations_are_removed_wherever_they_fall() {
        assert_eq!(strip("um so uh we should go"), "so we should go");
        assert_eq!(strip("we should, um, go"), "we should, go");
        assert_eq!(strip("Um, I think so"), "I think so");
    }

    #[test]
    fn elongated_hesitations_are_still_hesitations() {
        assert_eq!(strip("ummm hello uhhh there hmmm"), "hello there");
    }

    #[test]
    fn a_leading_hesitation_hands_its_capital_to_the_next_word() {
        assert_eq!(strip("Um so we begin"), "So we begin");
        assert_eq!(strip("Well. Um, we begin."), "Well. We begin.");
    }

    #[test]
    fn a_hesitation_that_ends_a_sentence_gives_its_stop_to_the_word_before() {
        assert_eq!(strip("I don't know, um. Next"), "I don't know. Next");
    }

    // ── "like" and its relatives ─────────────────────────────────────────

    #[test]
    fn like_as_a_verb_or_preposition_is_never_removed() {
        for kept in [
            "I like pizza",
            "it looks like rain",
            "something like that",
            "do you like it",
            "people like us",
            "I would like to go",
        ] {
            assert_eq!(strip(kept), kept, "must not touch: {kept}");
        }
    }

    #[test]
    fn like_set_off_by_commas_is_filler_and_goes() {
        assert_eq!(strip("it was, like, really good"), "it was, really good");
        assert_eq!(strip("Like, I have no idea"), "I have no idea");
        assert_eq!(strip("I went there, like."), "I went there.");
    }

    #[test]
    fn you_know_and_i_mean_need_the_punctuation_too() {
        assert_eq!(strip("It was fine, you know."), "It was fine.");
        assert_eq!(strip("You know, it was fine"), "It was fine");
        assert_eq!(strip("I mean, it was fine"), "It was fine");
        assert_eq!(strip("do you know the way"), "do you know the way");
        assert_eq!(strip("what I mean is simple"), "what I mean is simple");
        assert_eq!(strip("I mean it"), "I mean it");
    }

    #[test]
    fn kind_of_and_sort_of_are_kept_as_nouns_and_hedges_and_dropped_as_filler() {
        assert_eq!(strip("a kind of music"), "a kind of music");
        assert_eq!(strip("it was kind of good"), "it was kind of good");
        assert_eq!(strip("it was, kind of, good"), "it was, good");
    }

    #[test]
    fn meaningful_adverbs_go_only_when_bracketed_by_commas() {
        assert_eq!(strip("It was, basically, done"), "It was, done");
        assert_eq!(strip("Actually, I changed my mind"), "Actually, I changed my mind");
        assert_eq!(strip("I actually like it"), "I actually like it");
        assert_eq!(strip("it is literally on fire"), "it is literally on fire");
    }

    // ── safety ───────────────────────────────────────────────────────────

    #[test]
    fn words_that_merely_contain_a_filler_are_untouched() {
        assert_eq!(strip("an umbrella is likely useful, errands too"), "an umbrella is likely useful, errands too");
    }

    #[test]
    fn quoted_speech_is_left_alone() {
        assert_eq!(strip("he said \"um, like, no\" twice"), "he said \"um, like, no\" twice");
    }

    #[test]
    fn dictionary_terms_are_protected() {
        let band = [protect("like", "Like")];
        assert_eq!(strip_fillers("Like, I love Like", Some(&en()), &band), "Like, I love Like");
        let hum = [protect("uh", "UH")];
        assert_eq!(strip_fillers("uh yes", Some(&en()), &hum), "uh yes");
    }

    #[test]
    fn a_disabled_dictionary_entry_does_not_protect_anything() {
        let mut off = protect("like", "Like");
        off.enabled = false;
        assert_eq!(strip_fillers("Like, I know", Some(&en()), &[off]), "I know");
    }

    #[test]
    fn other_languages_are_never_touched() {
        let hindi = LanguageCode("hi".into());
        assert_eq!(strip_fillers("um like, you know", Some(&hindi), &[]), "um like, you know");
        assert_eq!(strip_fillers("um like", None, &[]), "um like");
    }

    #[test]
    fn line_breaks_from_spoken_commands_survive() {
        assert_eq!(strip("um first line\n\nuh second line"), "first line\n\nsecond line");
    }

    #[test]
    fn nothing_left_is_an_empty_string_not_a_stray_mark() {
        assert_eq!(strip("um"), "");
        assert_eq!(strip("um, uh, er."), "");
        assert_eq!(strip(""), "");
    }

    #[test]
    fn text_without_filler_comes_back_unchanged_apart_from_spacing() {
        assert_eq!(strip("We should ship this today."), "We should ship this today.");
        assert_eq!(strip("  spaced   out  "), "spaced out");
    }

    #[test]
    fn a_long_transcript_stays_well_inside_the_millisecond_budget() {
        let text = "um so like, we should, you know, go. ".repeat(500);
        let started = std::time::Instant::now();
        let _ = strip(&text);
        assert!(started.elapsed() < std::time::Duration::from_millis(50));
    }
}
