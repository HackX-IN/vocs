/*!
 * SOURCE OF TRUTH KEYWORDS: split_sentences, sentence_ranges, worth_rewriting,
 *   MAX_MODEL_WORDS, abbreviation, clause_chunks
 * WHAT:  Cuts a transcript into the sentences the grammar model is fed, as byte
 *        ranges into the original, so the text between them can be kept verbatim.
 * WHY:   Ranges rather than a Vec<String>: the transcript holds newlines from a
 *        spoken "new line" and the user's spacing, and rebuilding it from
 *        trimmed sentences would flatten both. One sentence at a time, because
 *        the model is trained on single sentences, its cost grows with length,
 *        and a per-sentence cache is what lets the sentences already finished
 *        while the person is still talking be corrected ahead of the stop.
 *
 *        A full stop is not always a sentence end ("e.g.", "Dr.", "vs."), and a
 *        wrong cut hands the model half a sentence, which it will cheerfully
 *        "complete" — the guard rejects that, but the cheap defence is not to
 *        cut there. Sentences longer than the model copes with are split at
 *        commas and semicolons; one that cannot be is left alone.
 * WHERE: Called by pipeline/grammar/pass.rs.
 */

use std::ops::Range;

/// Past this the model's quality and speed both fall off.
const MAX_MODEL_WORDS: usize = 40;
/// A sentence this long with no clause boundary to cut at is not attempted.
const GIVE_UP_WORDS: usize = 80;
const MIN_WORDS: usize = 2;

const ABBREVIATIONS: &[&str] = &[
    "mr", "mrs", "ms", "dr", "prof", "sr", "jr", "vs", "etc", "inc", "ltd", "co", "st", "no", "approx",
    "e.g", "i.e", "u.s", "u.k", "a.m", "p.m",
];

fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Whether the dot ending `before` belongs to an abbreviation.
fn ends_with_abbreviation(before: &str) -> bool {
    let last = before.rsplit(char::is_whitespace).next().unwrap_or("").trim_start_matches(['(', '"', '\'']);
    let lower = last.to_lowercase();
    let lower = lower.trim_end_matches('.');
    // A lone letter is an initial ("J. Smith"); a known short form is an abbreviation.
    (lower.chars().count() == 1 && lower.chars().all(char::is_alphabetic))
        || ABBREVIATIONS.contains(&lower)
}

/**
 * WHAT:  Byte ranges of each sentence in `text`, in order, without the
 *        whitespace between them.
 * WHY:   A newline is a hard boundary whatever the punctuation, because a
 *        spoken "new line" is the person's own paragraph break.
 * WHERE: split_sentences, and tests.
 */
pub fn sentence_ranges(text: &str) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let mut ranges = Vec::new();
    let mut start: Option<usize> = None;
    let mut i = 0;

    let mut close = |start: &mut Option<usize>, end: usize| {
        if let Some(s) = start.take() {
            let trimmed_end = s + text[s..end].trim_end().len();
            if trimmed_end > s {
                ranges.push(s..trimmed_end);
            }
        }
    };

    while i < bytes.len() {
        let ch = text[i..].chars().next().unwrap_or(' ');
        let width = ch.len_utf8();

        if ch == '\n' {
            close(&mut start, i);
        } else if !ch.is_whitespace() {
            start.get_or_insert(i);
            if matches!(ch, '.' | '?' | '!') {
                // Swallow a run of terminators and closing quotes/brackets.
                let mut end = i + width;
                while let Some(next) = text[end..].chars().next() {
                    if matches!(next, '.' | '?' | '!' | '"' | '\'' | '\u{201d}' | '\u{2019}' | ')' | ']') {
                        end += next.len_utf8();
                    } else {
                        break;
                    }
                }
                let rest = &text[end..];
                let after = rest.chars().next();
                let boundary = match after {
                    None => true,
                    Some(c) if c.is_whitespace() => {
                        let following = rest.trim_start().chars().next();
                        // "vs. the" continues; "done. The" starts again. A '?' or
                        // '!' ends the sentence whatever follows.
                        ch != '.'
                            || (!ends_with_abbreviation(&text[..i])
                                && following.is_none_or(|c| !c.is_lowercase()))
                    }
                    Some(_) => false,
                };
                if boundary {
                    close(&mut start, end);
                    i = end;
                    continue;
                }
                i = end;
                continue;
            }
        }
        i += width;
    }
    close(&mut start, text.len());
    ranges
}

/// Splits one over-long sentence at commas and semicolons into pieces the model
/// can take. `None` when there is no clean way to.
fn clause_chunks(text: &str, base: usize) -> Option<Vec<Range<usize>>> {
    if word_count(text) > GIVE_UP_WORDS {
        return None;
    }
    let mut chunks = Vec::new();
    let mut chunk_start = 0;
    let mut words_in_chunk = 0;
    let mut offset = 0;

    for piece in text.split_inclusive([',', ';']) {
        let piece_words = word_count(piece);
        if words_in_chunk > 0 && words_in_chunk + piece_words > MAX_MODEL_WORDS {
            chunks.push(base + chunk_start..base + offset);
            chunk_start = offset;
            words_in_chunk = 0;
        }
        words_in_chunk += piece_words;
        offset += piece.len();
    }
    chunks.push(base + chunk_start..base + offset);

    // Each chunk is trimmed to its content, and a single clause that is still
    // too long defeats the attempt.
    let trimmed: Vec<Range<usize>> = chunks
        .into_iter()
        .map(|r| {
            let slice = &text[r.start - base..r.end - base];
            let lead = slice.len() - slice.trim_start().len();
            r.start + lead..r.start + lead + slice.trim().len()
        })
        .collect();
    trimmed
        .iter()
        .all(|r| word_count(&text[r.start - base..r.end - base]) <= MAX_MODEL_WORDS + 20)
        .then_some(trimmed)
}

/// Whether a stretch is prose the model can sensibly be asked about.
pub fn worth_rewriting(sentence: &str) -> bool {
    word_count(sentence) >= MIN_WORDS
        && sentence.chars().any(char::is_alphabetic)
        // Code and shell lines are not sentences, whatever the tone setting says.
        && !sentence.contains(['{', '}', '$', '`', '<', '>', '|', '\\'])
}

/**
 * WHAT:  The ranges to hand the model: sentences, with over-long ones cut at
 *        clauses and unsuitable ones dropped.
 * WHERE: pass.rs::correct and pass.rs::prefetch.
 */
pub fn split_sentences(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    for range in sentence_ranges(text) {
        let sentence = &text[range.clone()];
        if word_count(sentence) > MAX_MODEL_WORDS {
            if let Some(chunks) = clause_chunks(sentence, range.start) {
                out.extend(chunks.into_iter().filter(|r| worth_rewriting(&text[r.clone()])));
            }
        } else if worth_rewriting(sentence) {
            out.push(range);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pieces(text: &str) -> Vec<&str> {
        sentence_ranges(text).into_iter().map(|r| &text[r]).collect()
    }

    #[test]
    fn ordinary_sentences_split_at_their_stops() {
        assert_eq!(pieces("I went home. Then I slept! Did you?"), ["I went home.", "Then I slept!", "Did you?"]);
    }

    #[test]
    fn abbreviations_decimals_and_file_names_do_not_end_a_sentence() {
        assert_eq!(pieces("Ask Dr. Rao about it."), ["Ask Dr. Rao about it."]);
        assert_eq!(pieces("Use e.g. a hammer, vs. a mallet."), ["Use e.g. a hammer, vs. a mallet."]);
        assert_eq!(pieces("It costs 3.5 dollars and main.rs is open."), ["It costs 3.5 dollars and main.rs is open."]);
        assert_eq!(pieces("Talk to J. Smith tomorrow."), ["Talk to J. Smith tomorrow."]);
    }

    #[test]
    fn a_newline_is_always_a_boundary() {
        assert_eq!(pieces("First line\nSecond line."), ["First line", "Second line."]);
    }

    #[test]
    fn quotes_after_the_stop_stay_with_their_sentence() {
        assert_eq!(pieces("He said \"go.\" Then he left."), ["He said \"go.\"", "Then he left."]);
    }

    #[test]
    fn ranges_let_the_gaps_between_sentences_survive() {
        let text = "One.  Two.\n\nThree.";
        let ranges = sentence_ranges(text);
        assert_eq!(ranges.len(), 3);
        assert_eq!(&text[ranges[0].end..ranges[1].start], "  ");
        assert_eq!(&text[ranges[1].end..ranges[2].start], "\n\n");
    }

    #[test]
    fn text_without_a_stop_is_one_sentence() {
        assert_eq!(pieces("no punctuation at all here"), ["no punctuation at all here"]);
        assert!(sentence_ranges("   ").is_empty());
        assert!(sentence_ranges("").is_empty());
    }

    #[test]
    fn a_long_sentence_is_cut_at_clauses_and_a_clauseless_monster_is_left_alone() {
        let clause = "we looked at the thing and then we talked about it for a while longer, ";
        let long = format!("{}end.", clause.repeat(5));
        let ranges = split_sentences(&long);
        assert!(ranges.len() > 1);
        assert!(ranges.iter().all(|r| word_count(&long[r.clone()]) <= MAX_MODEL_WORDS + 20));

        let monster = "word ".repeat(120);
        assert!(split_sentences(monster.trim()).is_empty());
    }

    #[test]
    fn single_words_and_code_are_not_worth_the_model() {
        assert!(split_sentences("Okay.").is_empty());
        assert!(split_sentences("run `ls -la` now").is_empty());
        assert!(split_sentences("echo $HOME | grep x").is_empty());
        assert_eq!(split_sentences("I has a idea.").len(), 1);
    }

    #[test]
    fn non_ascii_text_never_panics_on_a_byte_boundary() {
        let text = "Café ouvert. Ça va très bien! 你好。再见";
        let _ = split_sentences(text);
        let _ = pieces(text);
    }
}
