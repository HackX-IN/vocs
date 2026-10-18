/*!
 * SOURCE OF TRUTH KEYWORDS: fix_grammar, article_agreement, verb_agreement,
 *   modal_of, grammar_rules, GrammarEdit, a_an
 * WHAT:  A small set of grammar fixes that are right essentially every time:
 *        "a idea" → "an idea", "she don't" → "she doesn't", "we was" → "we were",
 *        "they has" → "they have", "their going to" → "they're going to",
 *        "its a" → "it's a", "could of been" → "could have been".
 * WHY:   Chosen by what a spell-checker CANNOT do and a rule CAN. macOS's own
 *        NSSpellChecker was measured on twelve realistic speech errors and
 *        flagged one of them ("a idea"), while it marked "Tauri" and "main.rs"
 *        as misspelled — so it would damage exactly the text this app exists
 *        to protect. Real grammar repair (tense, agreement in long sentences)
 *        needs a language model and is a separate project; this is the part
 *        that is safe without one.
 *
 *        Precision over recall, deliberately. Every rule looks at two or three
 *        ADJACENT words with no punctuation between them, and refuses when:
 *          - either word is inside quotes (somebody's exact words),
 *          - the word is in the user's dictionary (their spelling wins),
 *          - the neighbour is a word that could make the pattern legitimate:
 *            "a" followed by "and" is a variable ("points a and b"), not an
 *            article; "World War I has" is not "I have"; "an hour" is correct.
 *        Anything it does not recognise is left alone. A missed fix is noise;
 *        a wrong fix changes what somebody said.
 *
 *        Opt-in, like spoken corrections, for the same stated reason: it can
 *        rewrite words, and a silent failure should be a choice. History's diff
 *        and "Use original" make any edit visible and reversible.
 *
 *        Every rule replaces ONE token and keeps its trailing punctuation and
 *        its capital, so the pass can never move a sentence boundary.
 * WHERE: Called by adapters/rules/mod.rs after the dictionary and before the
 *        casing rules, when `EnhanceContext.fix_grammar` is on. English only.
 */

use crate::types::{DictionaryEntry, LanguageCode};

use super::fillers::{is_english, protected_words, tokenise, Token};

/// Words that follow a bare "a"/"an" only when "a" is a LABEL, never an
/// article ("between a and b", "plan a or plan b", "a is larger than b").
/// Seeing one means the article rule must stay out of the way.
const NOT_AFTER_AN_ARTICLE: &[&str] = &[
    "and", "or", "as", "at", "is", "are", "am", "in", "into", "on", "of", "off", "if", "it", "its", "i",
    "up", "upon", "us", "out", "over", "under", "also", "all", "always", "already", "about", "above",
    "after", "again", "against", "among", "around", "any", "anyone", "anything", "each", "either",
    "else", "even", "ever", "every", "enough", "equals",
];

/// Words spelled with a leading consonant that start with a vowel SOUND.
const SILENT_H: &[&str] = &[
    "hour", "hours", "hourly", "honest", "honestly", "honesty", "honor", "honors", "honored", "honorable",
    "honour", "honours", "honoured", "honourable", "heir", "heirs", "heiress",
];

/// Words spelled with a leading vowel that start with a consonant SOUND.
const VOWEL_LETTER_CONSONANT_SOUND: &[&str] = &["one", "once", "ones", "oneself", "ouija"];

/// Past participles that, after "could/should/would/must/might + of", prove the
/// writer meant "have". "could of course" is legitimate English; this list is
/// what keeps it that way.
const PARTICIPLES_AFTER_OF: &[&str] = &[
    "been", "had", "done", "gone", "made", "taken", "said", "known", "seen", "got", "gotten", "come",
    "become", "thought", "wanted", "tried", "worked", "helped", "used", "given", "found",
];

const MODALS: &[&str] = &["could", "should", "would", "must", "might"];

/// Subjects that take "doesn't"/"is" in the third person singular.
const THIRD_SINGULAR: &[&str] = &["he", "she", "it"];
const PLURAL_SUBJECTS: &[&str] = &["we", "you", "they"];

/**
 * WHAT:  Applies the grammar fixes to a transcript, line by line.
 * WHY:   Returns the input untouched, byte for byte, when no rule fires, so
 *        enabling the setting cannot alter text that needed nothing. A newline
 *        is a sentence boundary the rules must not look across.
 * WHERE: adapters/rules/mod.rs.
 */
pub fn fix_grammar(
    text: &str,
    language: Option<&LanguageCode>,
    dictionary: &[DictionaryEntry],
) -> String {
    if !is_english(language) {
        return text.to_string();
    }

    let protected = protected_words(dictionary);
    text.split('\n')
        .map(|line| fix_line(line, &protected))
        .collect::<Vec<_>>()
        .join("\n")
}

fn fix_line(line: &str, protected: &[String]) -> String {
    let tokens = tokenise(line);
    let mut words: Vec<String> = tokens.iter().map(|t| format!("{}{}", t.core, t.tail)).collect();
    let mut changed = false;

    for at in 0..tokens.len() {
        if let Some((index, replacement)) = edit_at(&tokens, at, protected) {
            words[index] = replacement;
            changed = true;
        }
    }

    if changed {
        words.join(" ")
    } else {
        line.to_string()
    }
}

/// The edit the tokens starting at `at` call for: which token, and what it
/// becomes (trailing punctuation included).
fn edit_at(tokens: &[Token<'_>], at: usize, protected: &[String]) -> Option<(usize, String)> {
    let first = tokens.get(at)?;
    let second = tokens.get(at + 1)?;

    if !usable(first, protected) || !usable(second, protected) {
        return None;
    }

    let a = first.core.to_ascii_lowercase();
    let b = second.core.to_ascii_lowercase();
    // The left word must end its own run: "she, don't" is not "she don't".
    let adjacent = first.tail.is_empty();

    // Articles: the first word is the one that changes.
    if adjacent && (a == "a" || a == "an") {
        let wanted = article_for(second.core)?;
        if wanted == a {
            return None;
        }
        return Some((at, format!("{}{}", match_case(first.core, wanted), first.tail)));
    }

    if !adjacent {
        return None;
    }

    // Subject + verb: the second word is the one that changes.
    let fixed = if THIRD_SINGULAR.contains(&a.as_str()) && b == "don't" {
        Some("doesn't")
    } else if PLURAL_SUBJECTS.contains(&a.as_str()) && b == "was" {
        Some("were")
    } else if PLURAL_SUBJECTS.contains(&a.as_str()) && b == "has" {
        Some("have")
    } else if (PLURAL_SUBJECTS.contains(&a.as_str()) || a == "i") && b == "doesn't" {
        Some("don't")
    } else {
        None
    };
    if let Some(fixed) = fixed {
        return Some((at + 1, format!("{}{}", match_case(second.core, fixed), second.tail)));
    }

    // "its a / its the / its not / its been" → "it's …". Possessive "its" is
    // never followed by these, which is what makes the rule safe.
    if a == "its" && matches!(b.as_str(), "a" | "an" | "the" | "not" | "been") {
        return Some((at, format!("{}{}", match_case(first.core, "it's"), first.tail)));
    }

    // "their going to" → "they're going to". Only with "going to" after it:
    // "their going rate" and "their meeting" are possessives.
    if a == "their" && b == "going" {
        let third = tokens.get(at + 2)?;
        if usable(third, protected) && third.core.eq_ignore_ascii_case("to") && second.tail.is_empty() {
            return Some((at, format!("{}{}", match_case(first.core, "they're"), first.tail)));
        }
        return None;
    }

    // "could of been" → "could have been". Needs a participle after "of".
    if MODALS.contains(&a.as_str()) && b == "of" && second.tail.is_empty() {
        let third = tokens.get(at + 2)?;
        if usable(third, protected) && PARTICIPLES_AFTER_OF.contains(&third.core.to_ascii_lowercase().as_str()) {
            return Some((at + 1, format!("have{}", second.tail)));
        }
    }

    None
}

/// Whether a token may take part in a rule at all.
pub(super) fn usable(token: &Token<'_>, protected: &[String]) -> bool {
    !token.quoted
        && !token.opens_with_mark
        && !token.core.is_empty()
        && !protected.contains(&token.core.to_lowercase())
}

/**
 * WHAT:  "a" or "an" for the word that follows, or None when it cannot be told.
 * WHY:   Decided by SOUND, not spelling, and only where the spelling is
 *        reliable: a/e/i/o (minus "one", "once", "eu…"), the silent-h words, and
 *        unambiguous consonants. Words starting with u, x, or an h not in the
 *        list are left alone ("a university", "an umbrella", "a hotel" are all
 *        right and not worth a table), as is anything that is not a plain
 *        lowercase word — acronyms, hyphenated words, names, numbers.
 * WHERE: edit_at.
 */
fn article_for(next: &str) -> Option<&'static str> {
    // Checked on the ORIGINAL spelling: "MBA" and "Priya" are not plain words,
    // and lower-casing first would turn "an MBA" into "a MBA".
    let plain = next.len() >= 2 && next.bytes().all(|b| b.is_ascii_lowercase());
    if !plain || NOT_AFTER_AN_ARTICLE.contains(&next) {
        return None;
    }

    if SILENT_H.contains(&next) {
        return Some("an");
    }

    let first = next.as_bytes()[0];
    match first {
        b'a' | b'i' => Some("an"),
        b'e' => Some(if next.starts_with("eu") || next.starts_with("ewe") { "a" } else { "an" }),
        b'o' => Some(if VOWEL_LETTER_CONSONANT_SOUND.contains(&next) { "a" } else { "an" }),
        // A word with no vowel is an acronym someone typed in lowercase
        // ("sms", "lcd"): its sound is a letter name, so spelling is no guide.
        b'b' | b'c' | b'd' | b'f' | b'g' | b'j' | b'k' | b'l' | b'm' | b'n' | b'p' | b'q' | b'r' | b's' | b't'
        | b'v' | b'w' | b'y' | b'z'
            if next.bytes().any(|b| matches!(b, b'a' | b'e' | b'i' | b'o' | b'u')) && next != "nth" =>
        {
            Some("a")
        }
        _ => None,
    }
}

/// Gives `replacement` the capital of `original` when it had one.
pub(super) fn match_case(original: &str, replacement: &str) -> String {
    let mut chars = replacement.chars();
    match (original.chars().next(), chars.next()) {
        (Some(first), Some(head)) if first.is_uppercase() => head.to_uppercase().chain(chars).collect(),
        _ => replacement.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DictionaryId, MatchKind};

    fn fix(text: &str) -> String {
        fix_grammar(text, Some(&LanguageCode("en".into())), &[])
    }

    fn unchanged(text: &str) {
        assert_eq!(fix(text), text, "should have been left alone");
    }

    #[test]
    fn a_becomes_an_before_a_vowel_sound() {
        assert_eq!(fix("I have a idea about it"), "I have an idea about it");
        assert_eq!(fix("it is a honest mistake"), "it is an honest mistake");
        assert_eq!(fix("wait a hour"), "wait an hour");
        assert_eq!(fix("A idea came up"), "An idea came up");
    }

    #[test]
    fn an_becomes_a_before_a_consonant_sound() {
        assert_eq!(fix("I need an book"), "I need a book");
        assert_eq!(fix("she is an european"), "she is a european");
        assert_eq!(fix("it was an one time thing"), "it was a one time thing");
        assert_eq!(fix("An plan is forming"), "A plan is forming");
    }

    #[test]
    fn correct_articles_are_left_alone() {
        unchanged("I have an idea about a university and a hotel");
        unchanged("wait an hour for an umbrella");
        unchanged("a one time thing and a european trip");
        unchanged("an honest answer");
    }

    #[test]
    fn a_used_as_a_label_is_not_an_article() {
        unchanged("the distance between a and b is short");
        unchanged("choose plan a or plan b");
        unchanged("a is larger than b");
        unchanged("press a in the menu");
    }

    #[test]
    fn acronyms_names_and_hyphenated_words_are_not_guessed() {
        unchanged("it is a SMS gateway");
        unchanged("an MBA programme");
        unchanged("a x-ray and an X-ray");
        unchanged("a lcd screen");
        unchanged("an Priya joined");
    }

    #[test]
    fn third_person_dont_becomes_doesnt() {
        assert_eq!(fix("she don't like the design"), "she doesn't like the design");
        assert_eq!(fix("He don't know"), "He doesn't know");
        assert_eq!(fix("it don't work, honestly"), "it doesn't work, honestly");
    }

    #[test]
    fn plural_doesnt_becomes_dont() {
        assert_eq!(fix("they doesn't agree"), "they don't agree");
        assert_eq!(fix("I doesn't mind"), "I don't mind");
    }

    #[test]
    fn plural_was_and_has_agree() {
        assert_eq!(fix("we was discussing it"), "we were discussing it");
        assert_eq!(fix("They was late"), "They were late");
        assert_eq!(fix("they has finished"), "they have finished");
        assert_eq!(fix("you was right, though"), "you were right, though");
    }

    #[test]
    fn i_has_is_deliberately_not_touched() {
        // "World War I has …" is a roman numeral, and the rule cannot tell.
        unchanged("World War I has a long history");
    }

    #[test]
    fn its_before_a_verb_phrase_becomes_its_contraction() {
        assert_eq!(fix("its a good plan"), "it's a good plan");
        assert_eq!(fix("Its not ready"), "It's not ready");
        assert_eq!(fix("its been a week"), "it's been a week");
        unchanged("the app and its a-b tests");
        unchanged("the company and its going rate");
        unchanged("the dog wagged its tail");
    }

    #[test]
    fn their_going_to_becomes_theyre_going_to() {
        assert_eq!(fix("their going to ship it"), "they're going to ship it");
        unchanged("their going rate is high");
        unchanged("their meeting went well");
    }

    #[test]
    fn modal_of_becomes_modal_have_only_before_a_participle() {
        assert_eq!(fix("we could of been faster"), "we could have been faster");
        assert_eq!(fix("it should of worked"), "it should have worked");
        assert_eq!(fix("you must of seen it"), "you must have seen it");
        unchanged("that could of course be true");
        unchanged("one of the best");
    }

    #[test]
    fn quoted_speech_is_never_edited() {
        unchanged(r#"he said "she don't care" and left"#);
        unchanged(r#"the sign read "a idea" in red"#);
    }

    #[test]
    fn a_punctuation_boundary_stops_a_rule() {
        unchanged("tell her and she, don't ask");
        unchanged("is that it? don't ask");
        unchanged("we saw it. don't worry");
    }

    #[test]
    fn dictionary_terms_win() {
        let dictionary = vec![DictionaryEntry {
            id: DictionaryId(1),
            pattern: "idea".into(),
            replacement: "Idea".into(),
            match_kind: MatchKind::Word,
            enabled: true,
            used_at: None,
        }];
        let out = fix_grammar("I have a Idea", Some(&LanguageCode("en".into())), &dictionary);
        assert_eq!(out, "I have a Idea");
    }

    #[test]
    fn other_languages_are_untouched() {
        let out = fix_grammar("ich habe a idea", Some(&LanguageCode("de".into())), &[]);
        assert_eq!(out, "ich habe a idea");
    }

    #[test]
    fn newlines_are_kept_and_not_looked_across() {
        assert_eq!(fix("she don't\nwe was there"), "she doesn't\nwe were there");
        unchanged("is it a\nidea");
    }

    #[test]
    fn the_pass_is_idempotent() {
        let once = fix("we was told she don't have a idea, its a mess");
        assert_eq!(fix(&once), once);
    }
}
