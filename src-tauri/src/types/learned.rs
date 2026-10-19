/*!
 * SOURCE OF TRUTH KEYWORDS: LearnedCorrection, LearnedId, CorrectionEdit,
 *   learned_mistakes, auto_apply, personal_mistake_dictionary
 * WHAT:  One remembered fix — "he go" becomes "he goes" — with the tally of how
 *        often the user kept it or undid it, and the edit record a session
 *        carries so those tallies can be settled later.
 * WHY:   A correction is learned from what the user DOES with it, not from what
 *        the model says: it counts as kept when the delivered text still holds
 *        the fix once the session has settled, and as undone when the user chose
 *        the original or edited it away. Only a fix kept three times and never
 *        undone is applied without the model (`auto_apply`), because the cost of
 *        a wrong auto-fix is silent, and one "no" must be enough to stop it.
 *        `manual` records that the user flipped the switch themselves, after
 *        which the tallies no longer move it — the person always outranks the
 *        arithmetic.
 * WHERE: Stored by services/learned.rs; produced by pipeline/grammar/guard.rs;
 *        settled by session/learning.rs; listed in Settings → Vocabulary.
 */

use serde::{Deserialize, Serialize};
use specta::Type;

use super::numeric::TsNumber;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
pub struct LearnedId(#[specta(type = TsNumber)] pub i64);

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LearnedCorrection {
    pub id: LearnedId,
    /// Lowercased words as they were said, with one word of context.
    pub pattern: String,
    pub replacement: String,
    pub accepted: u32,
    pub rejected: u32,
    /// Applied without asking the model.
    pub auto_apply: bool,
    /// The user set `auto_apply` themselves; the tallies leave it alone.
    pub manual: bool,
    #[specta(type = TsNumber)]
    pub updated_at: i64,
}

/// A fix as recorded on a session. Stored as JSON on the session row, never sent
/// over IPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrectionEdit {
    pub pattern: String,
    pub replacement: String,
    /// Applied from the learned list rather than proposed by the model.
    #[serde(default)]
    pub auto: bool,
}

/// Kept this many times, never undone: the point a fix applies itself.
pub const AUTO_APPLY_AFTER: u32 = 3;
