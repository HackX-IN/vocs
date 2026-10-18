/*!
 * SOURCE OF TRUTH KEYWORDS: word_diff, MAX_DIFF_WORDS, lcs_table,
 *   said_vs_fixed, DiffSegment
 * WHAT:  Compares a raw transcript with its corrected text word by word and
 *        returns runs of same / removed / added text.
 * WHY:   Every correction must be visible and reversible; you cannot trust a
 *        fix you cannot see. Word granularity, not character: a reader judges
 *        "um" → gone and "go" → "go." as whole tokens, and a character diff
 *        turns a changed comma into noise.
 *
 *        Longest-common-subsequence, which is quadratic, so it is bounded:
 *        past MAX_DIFF_WORDS on either side it declines to align and reports
 *        the whole raw text removed and the whole fixed text added. That is a
 *        true, if coarse, answer; a multi-second UI stall on a long dictation
 *        is not. The table is a flat Vec<u16>-sized grid, so the worst case is
 *        a few megabytes.
 *
 *        Words compare exactly, punctuation and case included: capitalising
 *        "so" to "So," IS a change the user should be able to see.
 * WHERE: Called by ipc/commands/history.rs (diff_history_entry).
 */

use crate::types::{DiffKind, DiffSegment};

/// Beyond this on either side, alignment is skipped. 1,500 words is about ten
/// minutes of speech, and 1,500² cells is a table of a few megabytes.
pub const MAX_DIFF_WORDS: usize = 1_500;

pub fn word_diff(raw: &str, fixed: &str) -> Vec<DiffSegment> {
    let said: Vec<&str> = raw.split_whitespace().collect();
    let kept: Vec<&str> = fixed.split_whitespace().collect();

    if said == kept {
        return push_run(Vec::new(), DiffKind::Same, &said);
    }
    if said.len() > MAX_DIFF_WORDS || kept.len() > MAX_DIFF_WORDS {
        let removed = push_run(Vec::new(), DiffKind::Removed, &said);
        return push_run(removed, DiffKind::Added, &kept);
    }

    let table = lcs_table(&said, &kept);
    let width = kept.len() + 1;
    let (mut i, mut j) = (0, 0);
    let mut out: Vec<DiffSegment> = Vec::new();

    while i < said.len() && j < kept.len() {
        if said[i] == kept[j] {
            out = push_run(out, DiffKind::Same, &said[i..=i]);
            i += 1;
            j += 1;
        // Prefer deleting before inserting, so a replaced word reads
        // "removed, added" in that order.
        } else if table[(i + 1) * width + j] >= table[i * width + j + 1] {
            out = push_run(out, DiffKind::Removed, &said[i..=i]);
            i += 1;
        } else {
            out = push_run(out, DiffKind::Added, &kept[j..=j]);
            j += 1;
        }
    }
    out = push_run(out, DiffKind::Removed, &said[i..]);
    push_run(out, DiffKind::Added, &kept[j..])
}

/// `table[i * (m + 1) + j]` is the LCS length of `a[i..]` and `b[j..]`.
fn lcs_table(a: &[&str], b: &[&str]) -> Vec<u16> {
    let width = b.len() + 1;
    let mut table = vec![0u16; (a.len() + 1) * width];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            table[i * width + j] = if a[i] == b[j] {
                table[(i + 1) * width + j + 1] + 1
            } else {
                table[(i + 1) * width + j].max(table[i * width + j + 1])
            };
        }
    }
    table
}

/// Appends words as one run, merging into the previous run of the same kind.
fn push_run(mut out: Vec<DiffSegment>, kind: DiffKind, words: &[&str]) -> Vec<DiffSegment> {
    if words.is_empty() {
        return out;
    }
    let text = words.join(" ");
    match out.last_mut() {
        Some(last) if last.kind == kind => {
            last.text.push(' ');
            last.text.push_str(&text);
        }
        _ => out.push(DiffSegment { kind, text }),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(kind: DiffKind, text: &str) -> DiffSegment {
        DiffSegment { kind, text: text.into() }
    }

    /// Rebuilds a side from the segments; the property every diff must hold.
    fn side(segments: &[DiffSegment], keep: DiffKind) -> String {
        segments
            .iter()
            .filter(|s| s.kind == DiffKind::Same || s.kind == keep)
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn identical_text_is_one_unchanged_run() {
        assert_eq!(word_diff("hello there", "hello  there"), vec![seg(DiffKind::Same, "hello there")]);
    }

    #[test]
    fn the_spec_example_shows_what_was_dropped_and_what_was_capitalised() {
        let diff = word_diff("um so uh we we should go", "So we should go.");
        assert_eq!(side(&diff, DiffKind::Removed), "um so uh we we should go");
        assert_eq!(side(&diff, DiffKind::Added), "So we should go.");
        assert!(diff.iter().any(|s| s.kind == DiffKind::Removed && s.text.contains("um")));
    }

    #[test]
    fn a_replaced_word_reads_removed_then_added() {
        assert_eq!(
            word_diff("send it today", "send it tomorrow"),
            vec![
                seg(DiffKind::Same, "send it"),
                seg(DiffKind::Removed, "today"),
                seg(DiffKind::Added, "tomorrow"),
            ]
        );
    }

    #[test]
    fn adjacent_changes_merge_into_single_runs() {
        let diff = word_diff("a um uh b", "a b");
        assert_eq!(
            diff,
            vec![seg(DiffKind::Same, "a"), seg(DiffKind::Removed, "um uh"), seg(DiffKind::Same, "b")]
        );
    }

    #[test]
    fn empty_sides_are_handled() {
        assert!(word_diff("", "").is_empty());
        assert_eq!(word_diff("", "new"), vec![seg(DiffKind::Added, "new")]);
        assert_eq!(word_diff("gone", ""), vec![seg(DiffKind::Removed, "gone")]);
    }

    #[test]
    fn both_sides_can_always_be_rebuilt_from_the_segments() {
        for (raw, fixed) in [
            ("the the quick brown fox", "The quick brown fox."),
            ("i dont know um", "I don't know."),
            ("one two three", "three two one"),
            ("same", "same"),
        ] {
            let diff = word_diff(raw, fixed);
            assert_eq!(side(&diff, DiffKind::Removed), raw.split_whitespace().collect::<Vec<_>>().join(" "));
            assert_eq!(side(&diff, DiffKind::Added), fixed.split_whitespace().collect::<Vec<_>>().join(" "));
        }
    }

    #[test]
    fn an_oversized_transcript_is_reported_coarsely_instead_of_stalling() {
        let long = "word ".repeat(MAX_DIFF_WORDS + 1);
        let started = std::time::Instant::now();
        let diff = word_diff(&long, "short");
        assert_eq!(diff.len(), 2);
        assert!(started.elapsed() < std::time::Duration::from_millis(100));
    }

    #[test]
    fn a_transcript_at_the_limit_still_aligns_within_a_sane_time() {
        let raw = "alpha bravo charlie ".repeat(MAX_DIFF_WORDS / 3);
        let fixed = "alpha charlie ".repeat(MAX_DIFF_WORDS / 3);
        let started = std::time::Instant::now();
        let diff = word_diff(&raw, &fixed);
        assert!(!diff.is_empty());
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
    }
}
