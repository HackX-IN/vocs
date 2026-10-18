/*!
 * SOURCE OF TRUTH KEYWORDS: BackupFile, BackupDictionaryEntry, ImportSummary,
 *   BACKUP_FORMAT, BACKUP_VERSION, backup_json, restore
 * WHAT:  The on-disk shape of a Vocs backup (dictionary and per-app profiles),
 *        and the tally an import reports.
 * WHY:   A backup is a contract with a FUTURE build, so it is versioned and
 *        carries a format tag: an unrelated JSON file, or a backup from a newer
 *        Vocs, is refused by name instead of being half-applied. Ids and usage
 *        timestamps are deliberately absent — they are properties of one
 *        machine's database, and carrying them would make a restore overwrite
 *        or collide with rows that mean something else on this Mac. Settings are
 *        absent too: the hotkey, input device and model are tied to a machine,
 *        and moving them to another would break that machine rather than sync
 *        it.
 * WHERE: Built and applied by ipc/commands/backup.rs; ImportSummary crosses to
 *        the Settings backup panel.
 */

use serde::{Deserialize, Serialize};
use specta::Type;

use super::dictionary::MatchKind;
use super::profile::AppProfile;

/// Written into every backup and required on import.
pub const BACKUP_FORMAT: &str = "vocs-backup";
/// Bumped only for a change an older reader cannot ignore.
pub const BACKUP_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupDictionaryEntry {
    pub pattern: String,
    pub replacement: String,
    pub match_kind: MatchKind,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFile {
    pub format: String,
    pub version: u32,
    pub exported_at: i64,
    /// Absent in a hand-edited file means "none", not "invalid".
    #[serde(default)]
    pub dictionary: Vec<BackupDictionaryEntry>,
    #[serde(default)]
    pub profiles: Vec<AppProfile>,
}

/// What an import changed and what it declined to. `skipped` counts entries
/// that were present but unusable (empty, over-long, invalid), so the user
/// learns the difference between "nothing to do" and "something was dropped".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ImportSummary {
    pub dictionary_added: u32,
    pub dictionary_updated: u32,
    pub dictionary_skipped: u32,
    pub profiles_added: u32,
    pub profiles_updated: u32,
    pub profiles_skipped: u32,
}

impl BackupFile {
    pub fn new(
        exported_at: i64,
        dictionary: Vec<BackupDictionaryEntry>,
        profiles: Vec<AppProfile>,
    ) -> Self {
        Self {
            format: BACKUP_FORMAT.to_string(),
            version: BACKUP_VERSION,
            exported_at,
            dictionary,
            profiles,
        }
    }
}
