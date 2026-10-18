/*!
 * SOURCE OF TRUTH KEYWORDS: dedupe_stutters, phrases_match, EMPHATIC_REPEATS,
 *   MAX_EMPHATIC_RUN, MAX_PHRASE_WORDS, repeated_words
 * WHAT:  Collapses an immediately repeated word or short phrase to one.
 * WHY:   Chunk boundaries overlap by design, so a word spoken across a seam can
 *        be decoded twice, and people genuinely stutter ("we we should go").
 *        Limited to ADJACENT repeats of up to three words: beyond that,
 *        repetition is real speech.
 *
 *        Three decisions, each from a case the first version got wrong:
 *          - A RUN collapses to one word, whatever its length. It used to
 *            collapse pairs, so "no no no" became "no no": half-fixed, which
 *            reads as a bug in either direction.
 *          - A few words are emphatic when doubled — "no no", "very very good",
 *            "really really" — and are left as spoken, up to three in a row.
 *            Collapsing them silently weakens what someone said, and the doc
 *            comment on the original claimed they were preserved when the code
 *            did the opposite.
 *          - Quoted speech is never edited. "They chanted "we we we"" is a
 *            quotation; the seam repair that matters happens earlier, in the
 *            assembler's overlap join, which does not depend on this pass.
 *        Matching ignores case and punctuation, so "hello, Hello" is a repeat.
 * WHERE: Runs after the dictionary in adapters/rules/mod.rs, so a corrected
 *        term is what gets compared.
 */

/// Longest repeated phrase collapsed. Three words covers a restarted clause
/// ("i want to i want to") without touching rhetorical repetition.
const MAX_PHRASE_WORDS: usize = 3;

/// A doubled word that is emphasis, not a stutter. Kept when the run is no
/// longer than MAX_EMPHATIC_RUN.
const EMPHATIC_REPEATS: &[&str] = &[
    "no", "yes", "very", "really", "ha", "go", "bye", "hey", "more", "oh", "wow", "please",
];
const MAX_EMPHATIC_RUN: usize = 3;

pub fn dedupe_stutters(text: &str) -> String {
    let words: Vec<&str> = text.split(' ').filter(|w| !w.is_empty()).collect();
    if words.len() < 2 {
        return text.to_string();
    }
    let quoted = quoted_flags(&words);
    let keys: Vec<String> = words.iter().map(|w| letters_only(w)).collect();

    let mut out: Vec<&str> = Vec::with_capacity(words.len());
    let mut index = 0usize;

    while index < words.len() {
        // A run of one word is decided FIRST. Checking phrases first read
        // "the the the the" as the pair "the the" repeated and kept two.
        let run = run_length(&keys, &quoted, index);
        if run > 1 {
            let emphatic = run <= MAX_EMPHATIC_RUN && EMPHATIC_REPEATS.contains(&keys[index].as_str());
            if emphatic {
                out.extend_from_slice(&words[index..index + run]);
            } else {
                out.push(words[index]);
            }
            index += run;
            continue;
        }

        // Longest phrase first, so "how do you how do you" collapses as a
        // phrase rather than leaving fragments behind.
        let phrase = (2..=MAX_PHRASE_WORDS).rev().find(|&span| {
            index + span * 2 <= words.len()
                && quoted[index..index + span * 2].iter().all(|q| !q)
                && phrases_match(&keys[index..index + span], &keys[index + span..index + span * 2])
        });
        if let Some(span) = phrase {
            out.extend_from_slice(&words[index..index + span]);
            index += span * 2;
        } else {
            out.push(words[index]);
            index += 1;
        }
    }

    out.join(" ")
}

/// How many words from `at` are the same word, as far as matching goes.
fn run_length(keys: &[String], quoted: &[bool], at: usize) -> usize {
    if keys[at].is_empty() || quoted[at] {
        return 1;
    }
    keys[at..]
        .iter()
        .zip(&quoted[at..])
        .take_while(|(key, quoted)| **key == keys[at] && !**quoted)
        .count()
}

fn phrases_match(a: &[String], b: &[String]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| !x.is_empty() && x == y)
}

/// Lowercased letters and digits, so "the" and "The," compare equal.
fn letters_only(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Which words sit inside, or open or close, a double-quoted run.
fn quoted_flags(words: &[&str]) -> Vec<bool> {
    let mut inside = false;
    words
        .iter()
        .map(|word| {
            let marks = word.chars().filter(|c| matches!(c, '"' | '\u{201C}' | '\u{201D}')).count();
            let flagged = inside || marks > 0;
            if marks % 2 == 1 {
                inside = !inside;
            }
            flagged
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_immediately_repeated_word_is_collapsed() {
        assert_eq!(dedupe_stutters("the the cat sat"), "the cat sat");
    }

    #[test]
    fn a_repeated_phrase_across_a_seam_is_collapsed_as_a_phrase() {
        // Exactly the chunk-overlap case: the last words of one chunk repeated
        // as the first words of the next.
        assert_eq!(dedupe_stutters("i want to i want to go home"), "i want to go home");
    }

    #[test]
    fn repetition_that_is_not_adjacent_is_preserved() {
        // Not adjacent duplicates of the same span — this is how people talk.
        assert_eq!(dedupe_stutters("very good and very bad"), "very good and very bad");
    }

    #[test]
    fn punctuation_does_not_hide_a_duplicate() {
        assert_eq!(dedupe_stutters("hello, hello there"), "hello, there");
        assert_eq!(dedupe_stutters("The the plan"), "The plan");
    }

    #[test]
    fn a_run_collapses_to_one_word_however_long() {
        assert_eq!(dedupe_stutters("we we we should go"), "we should go");
        assert_eq!(dedupe_stutters("the the the the plan"), "the plan");
    }

    #[test]
    fn emphasis_is_left_as_spoken() {
        assert_eq!(dedupe_stutters("no no no"), "no no no");
        assert_eq!(dedupe_stutters("very very good"), "very very good");
        assert_eq!(dedupe_stutters("really really"), "really really");
    }

    #[test]
    fn an_emphatic_word_repeated_past_three_is_a_stutter_after_all() {
        assert_eq!(dedupe_stutters("no no no no no"), "no");
    }

    #[test]
    fn quoted_speech_is_never_edited() {
        assert_eq!(
            dedupe_stutters("they chanted \"we we we\" all night"),
            "they chanted \"we we we\" all night"
        );
        // ...and the text around a quotation still is.
        assert_eq!(dedupe_stutters("the the \"a a\" ok"), "the \"a a\" ok");
    }

    #[test]
    fn nothing_and_one_word_pass_through() {
        assert_eq!(dedupe_stutters(""), "");
        assert_eq!(dedupe_stutters("hello"), "hello");
    }

    #[test]
    fn words_with_no_letters_are_never_treated_as_repeats() {
        assert_eq!(dedupe_stutters("- - -"), "- - -");
    }
}
