/*!
 * SOURCE OF TRUTH KEYWORDS: OrganizeMode, organize_mode, scratchpad_mode,
 *   from_stored
 * WHAT:  How the dictation scratchpad turns a ramble into something tidy: pick
 *        automatically, or force bullets, action items, or one clean paragraph.
 * WHY:   Crosses IPC (the scratchpad sends the mode it is showing) so it is the
 *        derived type, not a string the frontend and Rust each interpret. Auto is
 *        the default because "organise my ramble" should not first ask what shape
 *        the ramble is; the other three exist for when the person knows.
 * WHERE: Read by ipc/commands/scratchpad.rs, applied by adapters/rules/organize.rs
 *        through the TextOrganizer port, defaulted from the scratchpad.mode setting.
 */

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrganizeMode {
    /// Action items and notes when there are tasks, bullets for a longer
    /// ramble, a paragraph for a short one.
    #[default]
    Auto,
    /// One bullet per thought.
    Bullets,
    /// A checklist of what has to be done; says so when there is nothing to do.
    Actions,
    /// The same words as one clean paragraph.
    Paragraph,
}

impl OrganizeMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrganizeMode::Auto => "auto",
            OrganizeMode::Bullets => "bullets",
            OrganizeMode::Actions => "actions",
            OrganizeMode::Paragraph => "paragraph",
        }
    }

    /// An unrecognised stored value reads as Auto rather than failing.
    pub fn from_stored(value: &str) -> Self {
        match value {
            "bullets" => OrganizeMode::Bullets,
            "actions" => OrganizeMode::Actions,
            "paragraph" => OrganizeMode::Paragraph,
            _ => OrganizeMode::Auto,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_round_trips_through_its_stored_name() {
        for mode in [OrganizeMode::Auto, OrganizeMode::Bullets, OrganizeMode::Actions, OrganizeMode::Paragraph] {
            assert_eq!(OrganizeMode::from_stored(mode.as_str()), mode);
        }
        assert_eq!(OrganizeMode::from_stored("???"), OrganizeMode::Auto);
    }
}
