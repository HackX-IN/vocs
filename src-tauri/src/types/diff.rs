/*!
 * SOURCE OF TRUTH KEYWORDS: DiffSegment, DiffKind, word_diff, said_vs_fixed
 * WHAT:  One run of a word-level comparison between what was said (raw) and
 *        what was delivered (fixed): unchanged, removed, or added text.
 * WHY:   Segments are runs, not single words, so the UI renders a handful of
 *        spans per transcript instead of one per word, and a changed phrase
 *        reads as one removal next to one addition.
 * WHERE: Produced by pipeline/diff.rs; returned by diff_history_entry and drawn
 *        by the History detail panel.
 */

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiffKind {
    Same,
    /// In the original, gone from the fixed text.
    Removed,
    /// In the fixed text, not in the original.
    Added,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct DiffSegment {
    pub kind: DiffKind,
    pub text: String,
}
