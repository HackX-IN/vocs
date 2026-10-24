/*!
 * SOURCE OF TRUTH KEYWORDS: clean_segment, strip_non_speech_tags, collapse_repetition,
 *   CleanedSegment, MAX_LOOP_UNIT, REPEATS_TO_COLLAPSE, degenerate_decode
 * WHAT:  Cleans one decoded segment of the two things Whisper produces that are
 *        not words the user said: sound-effect tags ("*Scoffs*", "[BLANK_AUDIO]",
 *        "(laughs)", "♪") and runaway repetition ("the one who is the one who is…").
 * WHY:   Both were being PASTED. The History of a real machine held `*Scoffs*`,
 *        `*punch*` and `*Slow-Blood*` as whole dictations (each a breath or a
 *        click that the VAD let through), and a 2.4-second recording that came
 *        back as 217 words of one phrase on a loop.
 *
 *        The loop is a decoder failure, not a transcript: with temperature
 *        fallback pinned off for latency (docs/03 §2.2) nothing in whisper.cpp
 *        breaks it, so it runs to the token limit. Collapsing it is the safe
 *        half of the fix; the engine also uses `looped` to retry the decode once
 *        and to drop a result that loops twice, because a phrase that survives a
 *        second attempt came from noise, not from the user.
 *
 *        Deliberately conservative on both counts. A tag is removed only when it
 *        is short, wrapped in one of the marker pairs Whisper uses, and carries
 *        no digits — "(5)" and "[1]" are things people really dictate. A loop is
 *        only a loop at several consecutive repeats, so "very very good" and a
 *        genuine "no, no, no" survive; this never edits the middle of ordinary
 *        speech, only runs that no person produces.
 * WHERE: Applied per segment by adapters/whisper/engine.rs, before the
 *        hallucination blocklist (an all-tag segment is empty by then and the
 *        blocklist already drops empty text).
 */

/// Longest phrase, in tokens, that counts as one unit of a loop. Whisper's own
/// loops are a clause or a sentence; sixteen covers both without letting a long
/// ordinary sentence match itself.
const MAX_LOOP_UNIT: usize = 16;

/// Consecutive copies of a multi-token phrase before it is a loop.
const REPEATS_TO_COLLAPSE: usize = 3;

/// A single token repeated this many times in a row. Higher than the phrase
/// threshold because stutters and emphasis ("no no no") are real speech.
const SINGLE_TOKEN_REPEATS: usize = 5;

/// Longest text a marker pair may wrap and still be a sound tag. Anything longer
/// is a sentence that happens to be parenthesised.
const MAX_TAG_CHARS: usize = 40;

/// Most words a bracketed or parenthesised tag may hold ("(audience laughing)").
const MAX_TAG_WORDS: usize = 4;

/**
 * SOURCE OF TRUTH KEYWORDS: CleanedSegment
 * WHAT:  The cleaned text plus whether a repetition loop was found in it.
 * WHY:   `looped` is returned instead of being logged and forgotten because the
 *        engine acts on it: a looping decode is retried, and one that loops
 *        again is discarded.
 * WHERE: Returned by clean_segment to adapters/whisper/engine.rs.
 */
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanedSegment {
    pub text: String,
    pub looped: bool,
}

/**
 * WHAT:  Strips sound tags, then collapses repetition loops.
 * WHY:   In that order: a tag between two copies of a phrase would otherwise
 *        hide the loop from the matcher.
 * WHERE: adapters/whisper/engine.rs, once per decoded segment.
 */
pub fn clean_segment(text: &str) -> CleanedSegment {
    let stripped = strip_non_speech_tags(text);
    let (text, looped) = collapse_repetition(&stripped);
    CleanedSegment { text, looped }
}

/**
 * SOURCE OF TRUTH KEYWORDS: strip_non_speech_tags
 * WHAT:  Removes `*…*`, `[…]` and `(…)` annotations and music notes, and tidies
 *        the whitespace they leave behind.
 * WHY:   `*…*` is always an annotation in Whisper output — nobody dictates
 *        asterisks — so it is removed whatever it holds, up to the length cap.
 *        Brackets and parentheses are removed only when they hold 1–4 words and
 *        no digits, because those are the shapes of "[MUSIC]" and "(laughs)",
 *        while "(see above)" is rare enough to lose and "(2)" must survive.
 *        An unmatched opener is left alone: a half-tag is more likely the start
 *        of real text than a tag.
 * WHERE: clean_segment.
 */
pub fn strip_non_speech_tags(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;

    while index < chars.len() {
        let current = chars[index];

        if matches!(current, '♪' | '♫' | '♬') {
            index += 1;
            continue;
        }

        let closer = match current {
            '*' => Some('*'),
            '[' => Some(']'),
            '(' => Some(')'),
            _ => None,
        };

        if let Some(closer) = closer {
            let found = chars[index + 1..]
                .iter()
                .take(MAX_TAG_CHARS + 1)
                .position(|candidate| *candidate == closer);
            if let Some(offset) = found {
                let inner = &chars[index + 1..index + 1 + offset];
                if is_sound_tag(current, inner) {
                    index += offset + 2;
                    continue;
                }
            }
        }

        out.push(current);
        index += 1;
    }

    tidy_whitespace(&out)
}

/// Whether the text between a marker pair is an annotation rather than speech.
fn is_sound_tag(opener: char, inner: &[char]) -> bool {
    if inner.is_empty() || inner.len() > MAX_TAG_CHARS || inner.contains(&'\n') {
        return false;
    }
    if opener == '*' {
        return true;
    }
    let words = inner
        .iter()
        .collect::<String>()
        .split_whitespace()
        .count();
    (1..=MAX_TAG_WORDS).contains(&words) && !inner.iter().any(|c| c.is_ascii_digit())
}

/// Collapses the runs of space a removed tag leaves, and any space it leaves
/// before punctuation, then trims.
fn tidy_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space && !matches!(ch, '.' | ',' | '!' | '?' | ';' | ':') {
            out.push(' ');
        }
        pending_space = false;
        out.push(ch);
    }
    out
}

/// Lowercased letters and digits only, so two copies of a phrase match across
/// differing punctuation and casing. Empty for a pure-punctuation token.
fn token_key(token: &str) -> String {
    token
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/**
 * SOURCE OF TRUTH KEYWORDS: collapse_repetition
 * WHAT:  Replaces a phrase repeated several times in a row with one copy, and
 *        reports whether it did.
 * WHY:   Works on words, or on characters for scripts written without spaces,
 *        where a loop has no word boundaries to find. At each position it takes
 *        the candidate unit that covers the most text, so a four-word loop is
 *        not mistaken for a shorter unit nested inside it. The trailing partial
 *        copy a loop is usually cut off in ("…the one who is the one who.") is
 *        consumed with it, otherwise a fragment of the loop would be pasted.
 * WHERE: clean_segment.
 */
pub fn collapse_repetition(text: &str) -> (String, bool) {
    let spaced = text.split_whitespace().count() >= 2;
    let tokens: Vec<&str> = if spaced {
        text.split_whitespace().collect()
    } else {
        // `char_indices` windows keep the original bytes, so nothing is
        // re-encoded and the join below is exact.
        text.char_indices()
            .map(|(start, ch)| &text[start..start + ch.len_utf8()])
            .collect()
    };
    let keys: Vec<String> = tokens.iter().map(|token| token_key(token)).collect();

    let mut kept: Vec<&str> = Vec::with_capacity(tokens.len());
    let mut looped = false;
    let mut index = 0;

    while index < tokens.len() {
        match best_loop_at(&keys, index) {
            Some((unit, repeats)) => {
                kept.extend_from_slice(&tokens[index..index + unit]);
                let unit_start = index;
                index += unit * repeats;

                // The partial copy the decoder was cut off in the middle of.
                let mut offset = 0;
                while index < tokens.len()
                    && offset < unit
                    && !keys[index].is_empty()
                    && keys[index] == keys[unit_start + offset]
                {
                    index += 1;
                    offset += 1;
                }
                looped = true;
            }
            None => {
                kept.push(tokens[index]);
                index += 1;
            }
        }
    }

    if !looped {
        return (text.to_string(), false);
    }
    let separator = if spaced { " " } else { "" };
    (kept.join(separator), true)
}

/// The (unit length, repeat count) that covers the most tokens starting at
/// `start`, or None when nothing there repeats enough to be a loop.
fn best_loop_at(keys: &[String], start: usize) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize)> = None;

    for unit in 1..=MAX_LOOP_UNIT {
        if start + unit * 2 > keys.len() {
            break;
        }
        let pattern = &keys[start..start + unit];
        // A unit with an empty key is punctuation; never a phrase.
        if pattern.iter().any(String::is_empty) {
            continue;
        }

        let mut repeats = 1;
        while start + (repeats + 1) * unit <= keys.len()
            && keys[start + repeats * unit..start + (repeats + 1) * unit] == *pattern
        {
            repeats += 1;
        }

        let needed = if unit == 1 { SINGLE_TOKEN_REPEATS } else { REPEATS_TO_COLLAPSE };
        if repeats < needed {
            continue;
        }
        let covered = unit * repeats;
        if best.is_none_or(|(u, r)| covered > u * r) {
            best = Some((unit, repeats));
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_asterisk_annotation_is_removed_wherever_it_appears() {
        for tag in ["*Scoffs*", "*punch*", "*Slow-Blood*", "*sighs*"] {
            assert_eq!(strip_non_speech_tags(tag), "", "{tag} is not speech");
        }
        assert_eq!(strip_non_speech_tags("Okay. *Scoffs* Next."), "Okay. Next.");
        assert_eq!(strip_non_speech_tags("*laughs* that was good"), "that was good");
    }

    #[test]
    fn bracketed_sound_tags_go_but_numbered_references_stay() {
        assert_eq!(strip_non_speech_tags("[BLANK_AUDIO]"), "");
        assert_eq!(strip_non_speech_tags("[MUSIC]"), "");
        assert_eq!(strip_non_speech_tags("(audience laughing)"), "");
        assert_eq!(strip_non_speech_tags("see item (2) below"), "see item (2) below");
        assert_eq!(strip_non_speech_tags("as noted in [1] earlier"), "as noted in [1] earlier");
    }

    #[test]
    fn a_long_parenthetical_is_a_sentence_not_a_tag() {
        let said = "I went (and this is the long way round to the shop) yesterday";
        assert_eq!(strip_non_speech_tags(said), said);
    }

    #[test]
    fn an_unmatched_opener_is_kept() {
        assert_eq!(strip_non_speech_tags("a star * on its own"), "a star * on its own");
        assert_eq!(strip_non_speech_tags("open (bracket never closed"), "open (bracket never closed");
    }

    #[test]
    fn music_notes_are_dropped() {
        assert_eq!(strip_non_speech_tags("♪ la la ♪"), "la la");
    }

    #[test]
    fn the_real_loop_from_the_history_collapses_to_one_copy() {
        let looped = "Hello, I'm the one who is the one who is the one who is the one who is \
                      the one who is the one who is the one who is the one who is the one who.";
        let (text, found) = collapse_repetition(looped);
        assert!(found);
        assert_eq!(text, "Hello, I'm the one who is");
    }

    #[test]
    fn ordinary_repetition_is_left_alone() {
        for said in [
            "that is very very good",
            "no no no that is wrong",
            "go to the shop and then go to the shop again",
            "the plan is the plan",
        ] {
            let (text, found) = collapse_repetition(said);
            assert!(!found, "{said}");
            assert_eq!(text, said);
        }
    }

    #[test]
    fn a_single_word_stuck_on_repeat_collapses() {
        let (text, found) = collapse_repetition("so so so so so so so then we left");
        assert!(found);
        assert_eq!(text, "so then we left");
    }

    #[test]
    fn a_loop_in_a_script_without_spaces_collapses() {
        let (text, found) = collapse_repetition("ありがとうございますありがとうございますありがとうございますありがとうございます");
        assert!(found);
        assert_eq!(text, "ありがとうございます");
    }

    #[test]
    fn a_tag_hiding_between_copies_does_not_hide_the_loop() {
        let cleaned = clean_segment("thank you all *sighs* thank you all thank you all thank you all");
        assert!(cleaned.looped);
        assert_eq!(cleaned.text, "thank you all");
    }

    #[test]
    fn clean_text_passes_through_unchanged_and_unflagged() {
        let cleaned = clean_segment("Can we improve the performance and make it fast when I speak and stop?");
        assert!(!cleaned.looped);
        assert_eq!(cleaned.text, "Can we improve the performance and make it fast when I speak and stop?");
    }
}
