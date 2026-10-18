/*!
 * SOURCE OF TRUTH KEYWORDS: capitalise_sentences, ensure_terminal_punctuation,
 *   ends_abbreviation, sentence_boundary, TITLE_ABBREVIATIONS
 * WHAT:  Capitalises the start of each sentence, and adds a full stop to text
 *        that has none.
 * WHY:   Whisper is trained on punctuated text and usually gets this right, so
 *        this is a repair pass, and it only ever UPPER-cases a leading letter —
 *        lowering anything would destroy the proper nouns and acronyms the
 *        model got right.
 *
 *        A sentence boundary is a stop FOLLOWED BY WHITESPACE. The first version
 *        treated any "." as a boundary, so for a tool people dictate code and
 *        prose into it produced "main.Rs", "example.Com", "whisper.Cpp" and
 *        "version 2.5.1 Is out". A dot with a letter or digit directly after it
 *        is a file extension, a domain or a version, never a sentence.
 *        Initialisms ("p.m.", "e.g.", "U.S.") and titles ("Dr.", "vs.") end in a
 *        dot without ending the sentence, so the word after them is left alone;
 *        the cost is a missed capital when one really does end a sentence,
 *        which is the cheaper mistake.
 * WHERE: Steps 7 and 8 of adapters/rules/mod.rs: casing, then the terminal stop.
 */

/// Abbreviations that end in a dot without ending the sentence.
const TITLE_ABBREVIATIONS: &[&str] = &["dr.", "mr.", "mrs.", "ms.", "prof.", "st.", "vs.", "etc"];

/// Marks that close a sentence's punctuation without being part of the next
/// sentence: `He left." Then` still starts a new sentence after the quote.
fn is_closer(ch: char) -> bool {
    matches!(ch, '"' | '\'' | ')' | ']' | '\u{201D}' | '\u{2019}')
}

/// True when the whitespace-delimited word ending at the end of `text` is an
/// abbreviation rather than the end of a sentence.
fn ends_abbreviation(text: &str) -> bool {
    let word = text
        .rsplit(char::is_whitespace)
        .next()
        .unwrap_or("")
        .trim_start_matches(|c: char| !c.is_alphanumeric())
        .trim_end_matches(is_closer)
        .to_lowercase();

    if TITLE_ABBREVIATIONS.contains(&word.as_str()) {
        return true;
    }
    // "p.m.", "e.g.", "u.s." — at least two single letters, each followed by a dot.
    let parts: Vec<&str> = word.trim_end_matches('.').split('.').collect();
    word.ends_with('.')
        && parts.len() >= 2
        && parts.iter().all(|part| part.chars().count() == 1 && part.chars().all(char::is_alphabetic))
}

pub fn capitalise_sentences(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut capitalise_next = true;
    // A stop has been seen and nothing but closers since: if whitespace comes
    // next, a sentence has ended.
    let mut stop_pending = false;

    for ch in text.chars() {
        if capitalise_next && ch.is_alphabetic() {
            out.extend(ch.to_uppercase());
            capitalise_next = false;
            stop_pending = false;
            continue;
        }

        out.push(ch);
        match ch {
            '\n' => {
                capitalise_next = true;
                stop_pending = false;
            }
            '.' | '!' | '?' => stop_pending = true,
            c if c.is_whitespace() => {
                if stop_pending {
                    // `out` ends with the whitespace just pushed; look before it.
                    let before = &out[..out.len() - c.len_utf8()];
                    if !(before.ends_with('.') && ends_abbreviation(before)) {
                        capitalise_next = true;
                    }
                    stop_pending = false;
                }
            }
            c if is_closer(c) => {}
            // Anything else directly after a stop — a letter, a digit — means it
            // was inside a token ("main.rs"), not between sentences.
            _ => stop_pending = false,
        }
    }

    out
}

/**
 * WHAT:  Adds a full stop if the text does not already end in punctuation.
 * WHY:   Skipped when the text ends in a newline from an explicit "new
 *        paragraph" command — the user was structuring, not finishing a
 *        sentence. A closing quote counts as clean (where the stop belongs
 *        depends on a style choice we should not make for the user), but a
 *        closing bracket does not: "call me (later)" wants its stop after the
 *        bracket unless one is already inside it.
 */
pub fn ensure_terminal_punctuation(text: &str) -> String {
    let trimmed = text.trim_end();
    if trimmed.is_empty() {
        return text.to_string();
    }

    let stripped = trimmed.trim_end_matches(')');
    let ends_cleanly = match trimmed.chars().last() {
        Some('"') => true,
        // Bracketed text is finished only if the stop sits inside the bracket.
        Some(')') => stripped.ends_with(['.', '!', '?', ':', ';', ',']),
        Some(c) => matches!(c, '.' | '!' | '?' | ':' | ';' | ','),
        None => false,
    };

    if ends_cleanly || text.ends_with('\n') {
        return text.to_string();
    }

    format!("{trimmed}.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentences_are_capitalised_without_lowercasing_anything() {
        assert_eq!(capitalise_sentences("hello there. how are you?"), "Hello there. How are you?");
        // Acronyms and proper nouns the model got right must survive.
        assert_eq!(capitalise_sentences("the API is fine"), "The API is fine");
    }

    #[test]
    fn a_dot_inside_a_token_is_not_a_sentence_boundary() {
        assert_eq!(capitalise_sentences("open main.rs now"), "Open main.rs now");
        assert_eq!(capitalise_sentences("visit example.com today"), "Visit example.com today");
        assert_eq!(capitalise_sentences("version 2.5.1 is out"), "Version 2.5.1 is out");
        assert_eq!(capitalise_sentences("use whisper.cpp here"), "Use whisper.cpp here");
        assert_eq!(capitalise_sentences("mail dev@example.com please"), "Mail dev@example.com please");
    }

    #[test]
    fn a_stop_then_a_space_still_starts_a_new_sentence() {
        assert_eq!(capitalise_sentences("done. next"), "Done. Next");
        assert_eq!(capitalise_sentences("really? yes! ok."), "Really? Yes! Ok.");
        assert_eq!(capitalise_sentences("one.  two"), "One.  Two");
    }

    #[test]
    fn closing_quotes_and_brackets_do_not_hide_a_boundary() {
        assert_eq!(capitalise_sentences("he left.\" then she did"), "He left.\" Then she did");
        assert_eq!(capitalise_sentences("(it ended.) next"), "(It ended.) Next");
    }

    #[test]
    fn initialisms_and_titles_do_not_end_a_sentence() {
        assert_eq!(capitalise_sentences("meet at 3 p.m. tomorrow"), "Meet at 3 p.m. tomorrow");
        assert_eq!(capitalise_sentences("tools e.g. cargo work"), "Tools e.g. cargo work");
        assert_eq!(capitalise_sentences("ask dr. patel today"), "Ask dr. patel today");
        assert_eq!(capitalise_sentences("made in the u.s. last year"), "Made in the u.s. last year");
    }

    #[test]
    fn a_single_letter_before_a_stop_is_still_a_sentence_end() {
        // "Plan B." is a sentence; only multi-letter initialisms are skipped.
        assert_eq!(capitalise_sentences("we chose plan b. then we left"), "We chose plan b. Then we left");
    }

    #[test]
    fn a_new_line_starts_a_new_sentence() {
        assert_eq!(capitalise_sentences("first\nsecond"), "First\nSecond");
    }

    #[test]
    fn text_that_is_not_letters_passes_through() {
        assert_eq!(capitalise_sentences(""), "");
        assert_eq!(capitalise_sentences("123 456"), "123 456");
    }

    #[test]
    fn terminal_punctuation_is_added_only_when_missing() {
        assert_eq!(ensure_terminal_punctuation("hello"), "hello.");
        assert_eq!(ensure_terminal_punctuation("hello."), "hello.");
        assert_eq!(ensure_terminal_punctuation("really?"), "really?");
        assert_eq!(ensure_terminal_punctuation(""), "");
    }

    #[test]
    fn a_deliberate_trailing_newline_does_not_get_a_full_stop() {
        assert_eq!(ensure_terminal_punctuation("a list\n\n"), "a list\n\n");
    }

    #[test]
    fn a_closing_bracket_gets_its_stop_unless_one_is_inside() {
        assert_eq!(ensure_terminal_punctuation("call me (later)"), "call me (later).");
        assert_eq!(ensure_terminal_punctuation("call me (later.)"), "call me (later.)");
        assert_eq!(ensure_terminal_punctuation("see (a) and (b)"), "see (a) and (b).");
    }

    #[test]
    fn a_closing_quote_is_left_alone() {
        assert_eq!(ensure_terminal_punctuation("he said \"hello\""), "he said \"hello\"");
    }
}
