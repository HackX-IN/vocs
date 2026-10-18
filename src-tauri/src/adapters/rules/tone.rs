/*!
 * SOURCE OF TRUTH KEYWORDS: formalise, relax_ending, tone_rules, contractions, informal_shortcuts,
 *   casual_ending, formal_register
 * WHAT:  The two tone passes that rewrite words: `formalise` writes contractions
 *        out ("don't" → "do not") and spells spoken shortcuts ("gonna" → "going
 *        to"); `relax_ending` drops the full stop from a one-line message.
 * WHY:   A tone is a register, and register is mostly a handful of
 *        deterministic habits — which is why it needs no language model. Both
 *        passes keep the same discipline as grammar.rs: precision over recall,
 *        every rule replaces ONE token and keeps its trailing punctuation and its
 *        capital, and anything inside quotes or in the user's dictionary is left
 *        alone. A missed expansion is a slightly casual email; a wrong one changes
 *        what somebody said.
 *
 *        THE AMBIGUOUS ONES ARE LEFT OUT. "'d" is "would" or "had", "'s" is "is",
 *        "has" or a possessive, "let's" is not a contraction of anything you
 *        would write out. "it's" is expanded because it can only be "it is" or
 *        "it has", and the word after it decides which ("it's been" is "it has
 *        been"). Everything in the table below is a form with exactly one
 *        reading.
 *
 *        `relax_ending` is deliberately timid: it only touches a message that is
 *        ONE sentence on ONE line, because "Sounds good" is how a person types
 *        in chat and "Sounds good. See you at eight" with the middle full stop
 *        kept and the last dropped would be a half-edit. It leaves "?" and "!"
 *        alone — they are the tone — and refuses an ending that belongs to an
 *        abbreviation ("etc.", "e.g.").
 * WHERE: adapters/rules/mod.rs — `formalise` after the grammar pass, `relax_ending`
 *        after the terminal stop. Both are no-ops for Tone::Natural.
 */

use crate::types::{DictionaryEntry, LanguageCode};

use super::fillers::{is_english, protected_words, tokenise};
use super::grammar::{match_case, usable};

/// Forms with exactly one reading, and what they become.
const EXPANSIONS: &[(&str, &str)] = &[
    ("don't", "do not"),
    ("doesn't", "does not"),
    ("didn't", "did not"),
    ("can't", "cannot"),
    ("won't", "will not"),
    ("isn't", "is not"),
    ("aren't", "are not"),
    ("wasn't", "was not"),
    ("weren't", "were not"),
    ("haven't", "have not"),
    ("hasn't", "has not"),
    ("hadn't", "had not"),
    ("wouldn't", "would not"),
    ("couldn't", "could not"),
    ("shouldn't", "should not"),
    ("mustn't", "must not"),
    ("i'm", "I am"),
    ("i've", "I have"),
    ("i'll", "I will"),
    ("you're", "you are"),
    ("you've", "you have"),
    ("you'll", "you will"),
    ("we're", "we are"),
    ("we've", "we have"),
    ("we'll", "we will"),
    ("they're", "they are"),
    ("they've", "they have"),
    ("they'll", "they will"),
    // Spoken shortcuts.
    ("gonna", "going to"),
    ("wanna", "want to"),
    ("gotta", "have to"),
    ("kinda", "kind of"),
    ("sorta", "sort of"),
    ("lemme", "let me"),
    ("gimme", "give me"),
    ("dunno", "do not know"),
    ("yeah", "yes"),
    ("yep", "yes"),
    ("yup", "yes"),
    ("nope", "no"),
];

/// "X's" forms that are "X is" — or "X has" when a past participle follows.
const IS_OR_HAS: &[&str] = &["it's", "that's", "there's", "here's", "what's", "who's", "where's"];

/// What after "it's" proves it was "it has" ("it's been", "it's got").
const HAS_FOLLOWERS: &[&str] = &["been", "got", "gotten", "had", "become", "taken", "made", "done"];

/// Endings that belong to an abbreviation, not to a sentence.
const ABBREVIATIONS: &[&str] = &["etc", "vs", "inc", "ltd", "co", "mr", "mrs", "ms", "dr", "st", "no", "approx"];

/// Straight and curly apostrophes are the same letter to the rules.
fn plain_apostrophe(word: &str) -> String {
    word.replace('\u{2019}', "'").to_lowercase()
}

/**
 * WHAT:  Writes contractions and spoken shortcuts out, line by line.
 * WHY:   Returns the input byte for byte when nothing fires, so Formal cannot
 *        alter text that needed nothing. English only: the table is English, and
 *        an expansion applied to another language's word would be nonsense.
 * WHERE: adapters/rules/mod.rs.
 */
pub fn formalise(text: &str, language: Option<&LanguageCode>, dictionary: &[DictionaryEntry]) -> String {
    if !is_english(language) {
        return text.to_string();
    }
    let protected = protected_words(dictionary);
    text.split('\n')
        .map(|line| formalise_line(line, &protected))
        .collect::<Vec<_>>()
        .join("\n")
}

fn formalise_line(line: &str, protected: &[String]) -> String {
    let tokens = tokenise(line);
    let mut words: Vec<String> = tokens.iter().map(|t| format!("{}{}", t.core, t.tail)).collect();
    let mut changed = false;

    for (index, token) in tokens.iter().enumerate() {
        if !usable(token, protected) {
            continue;
        }
        // ALL-CAPS is shouting or an acronym, not a register to rewrite.
        if token.core.len() > 1 && token.core.chars().all(|c| !c.is_lowercase()) {
            continue;
        }
        let lowered = plain_apostrophe(token.core);

        let expanded: Option<String> = if IS_OR_HAS.contains(&lowered.as_str()) {
            let has = tokens
                .get(index + 1)
                .map(|next| plain_apostrophe(next.core))
                .is_some_and(|next| HAS_FOLLOWERS.contains(&next.as_str()));
            // "it's" → "it is" / "it has": built from the stem so the table does
            // not need a pair of rows per word.
            Some(format!("{} {}", lowered.trim_end_matches("'s"), if has { "has" } else { "is" }))
        } else {
            EXPANSIONS.iter().find(|(from, _)| *from == lowered).map(|(_, to)| (*to).to_string())
        };

        if let Some(expanded) = expanded {
            words[index] = format!("{}{}", match_case(token.core, &expanded), token.tail);
            changed = true;
        }
    }

    if changed {
        words.join(" ")
    } else {
        line.to_string()
    }
}

/**
 * WHAT:  Removes the final full stop from a message that is one sentence on one
 *        line.
 * WHY:   See the module WHY. A text with a second sentence, a second line, a
 *        question mark or exclamation mark ending, or an abbreviation at the end
 *        is returned unchanged.
 * WHERE: adapters/rules/mod.rs, after the terminal stop has been added.
 */
pub fn relax_ending(text: &str) -> String {
    let trimmed = text.trim_end();
    if trimmed.contains('\n') || !trimmed.ends_with('.') || trimmed.ends_with("..") {
        return text.to_string();
    }

    let body = &trimmed[..trimmed.len() - 1];
    if body.trim().is_empty() || has_sentence_boundary(body) {
        return text.to_string();
    }

    let last = body.split_whitespace().next_back().unwrap_or_default();
    let last_word = last.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    // "e.g" ends in a dot that is part of the word. "3.5" does not: a number with
    // a decimal point still ends a sentence like any other.
    if (last.contains('.') && last.chars().any(char::is_alphabetic)) || ABBREVIATIONS.contains(&last_word.as_str()) {
        return text.to_string();
    }

    format!("{body}{}", &text[trimmed.len()..])
}

/// Whether `body` holds a sentence end before its last word: ". " / "? " / "! ".
fn has_sentence_boundary(body: &str) -> bool {
    let chars: Vec<char> = body.chars().collect();
    chars.windows(2).any(|pair| matches!(pair[0], '.' | '!' | '?') && pair[1].is_whitespace())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DictionaryId, MatchKind};

    fn formal(text: &str) -> String {
        formalise(text, Some(&LanguageCode("en".into())), &[])
    }

    #[test]
    fn contractions_are_written_out_and_the_capital_survives() {
        assert_eq!(formal("I don't think we can't"), "I do not think we cannot");
        assert_eq!(formal("Don't worry."), "Do not worry.");
        assert_eq!(formal("I'm sure they're coming."), "I am sure they are coming.");
        assert_eq!(formal("We won't be late, and I'll call."), "We will not be late, and I will call.");
    }

    #[test]
    fn spoken_shortcuts_are_spelled_out() {
        assert_eq!(formal("I'm gonna send it, yeah"), "I am going to send it, yes");
        assert_eq!(formal("we gotta go"), "we have to go");
        assert_eq!(formal("kinda late, dunno"), "kind of late, do not know");
    }

    #[test]
    fn its_is_it_is_unless_a_participle_follows() {
        assert_eq!(formal("it's ready"), "it is ready");
        assert_eq!(formal("it's been a week"), "it has been a week");
        assert_eq!(formal("there's a problem"), "there is a problem");
        assert_eq!(formal("there's been a change"), "there has been a change");
        assert_eq!(formal("That's fine."), "That is fine.");
    }

    #[test]
    fn trailing_punctuation_is_kept_on_the_expanded_word() {
        assert_eq!(formal("I can't, honestly."), "I cannot, honestly.");
        assert_eq!(formal("No, I don't."), "No, I do not.");
    }

    #[test]
    fn the_curly_apostrophe_is_the_same_letter() {
        assert_eq!(formal("I don\u{2019}t know"), "I do not know");
    }

    #[test]
    fn the_ambiguous_forms_are_left_alone() {
        // 'd is would/had, 's is is/has/possessive, let's is not a contraction
        // anyone writes out.
        assert_eq!(formal("he'd go and John's car, let's see"), "he'd go and John's car, let's see");
    }

    #[test]
    fn quoted_speech_and_dictionary_words_are_never_touched() {
        assert_eq!(formal(r#"she said "I don't know" and left"#), r#"she said "I don't know" and left"#);
        let entry = DictionaryEntry {
            id: DictionaryId(1),
            pattern: "gonna".into(),
            replacement: "Gonna".into(),
            match_kind: MatchKind::Word,
            enabled: true,
            used_at: None,
        };
        assert_eq!(
            formalise("gonna", Some(&LanguageCode("en".into())), &[entry]),
            "gonna",
            "the user's own spelling wins"
        );
    }

    #[test]
    fn shouting_and_acronyms_are_not_a_register() {
        assert_eq!(formal("DON'T PANIC"), "DON'T PANIC");
    }

    #[test]
    fn other_languages_are_untouched() {
        let french = LanguageCode("fr".into());
        assert_eq!(formalise("don't gonna", Some(&french), &[]), "don't gonna");
    }

    #[test]
    fn text_that_needs_nothing_comes_back_byte_for_byte() {
        let text = "This is  already   formal.\nSecond line.";
        assert_eq!(formal(text), text);
    }

    #[test]
    fn a_one_line_message_loses_its_full_stop() {
        assert_eq!(relax_ending("Sounds good."), "Sounds good");
        assert_eq!(relax_ending("See you at 5."), "See you at 5");
    }

    #[test]
    fn a_second_sentence_a_question_or_a_second_line_keeps_everything() {
        assert_eq!(relax_ending("Sounds good. See you there."), "Sounds good. See you there.");
        assert_eq!(relax_ending("Is that okay?"), "Is that okay?");
        assert_eq!(relax_ending("Great!"), "Great!");
        assert_eq!(relax_ending("First line.\nSecond line."), "First line.\nSecond line.");
        assert_eq!(relax_ending("Wait..."), "Wait...");
    }

    #[test]
    fn an_abbreviation_ending_is_not_a_sentence_ending() {
        assert_eq!(relax_ending("Bring snacks, drinks, etc."), "Bring snacks, drinks, etc.");
        assert_eq!(relax_ending("Use e.g."), "Use e.g.");
        assert_eq!(relax_ending("It costs 3.5."), "It costs 3.5");
    }

    #[test]
    fn nothing_is_left_when_only_a_stop_remains() {
        assert_eq!(relax_ending("."), ".");
        assert_eq!(relax_ending(""), "");
    }
}
