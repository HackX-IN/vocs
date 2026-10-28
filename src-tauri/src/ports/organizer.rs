/*!
 * SOURCE OF TRUTH KEYWORDS: TextOrganizer, organize, scratchpad_organizer
 * WHAT:  The trait that turns a rambling block of text into bullets, a task list
 *        or a clean paragraph.
 * WHY:   A port of its own and not a mode of TextEnhancer. The enhancer is
 *        sentence-level polish that runs on every dictation inside the finalize
 *        budget; this restructures a whole document, runs only when the person
 *        asks, and is exactly the job a local language model would do better one
 *        day. Keeping it behind a trait means that day is a new adapter, not a
 *        change to the scratchpad.
 * WHERE: Implemented by adapters/rules/organize.rs; called by
 *        ipc/commands/scratchpad.rs.
 */

use crate::error::AppResult;
use crate::types::{LanguageCode, OrganizeMode};

pub trait TextOrganizer: Send + Sync {
    fn id(&self) -> &'static str;

    /// Restructures `text` per `mode`. `language` is the language it was spoken
    /// in when known; an implementation that only understands some languages
    /// returns the text tidied but otherwise unchanged for the others. Errors
    /// carry a sentence the person can act on ("No action items found.").
    fn organize(&self, text: &str, mode: OrganizeMode, language: Option<&LanguageCode>) -> AppResult<String>;
}
