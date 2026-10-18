/*!
 * SOURCE OF TRUTH KEYWORDS: export_backup, import_backup, ImportBackupInput,
 *   sanitise_backup, parse_backup, MAX_BACKUP_BYTES, apply_backup
 * WHAT:  Builds a JSON backup of the dictionary and per-app profiles, and
 *        restores one, merging it into what is already there.
 * WHY:   Two Macs, one set of custom words, no cloud. The import is a MERGE, not
 *        a replace: restoring a backup onto a machine that has grown its own
 *        entries must never delete them, so it only adds and updates. Matching
 *        is on the same key the database enforces (pattern + match kind;
 *        bundle id), which makes a restore idempotent — running it twice, or
 *        re-running it after a failure part-way through, converges on the same
 *        state, and that is what stands in for a cross-table transaction here
 *        (services are one table each and may not call each other).
 *
 *        An imported file is untrusted input however it got here, so every
 *        entry goes through the same rules as one typed into the form
 *        (check_entry_text, SettingDef::validate_value) and anything failing
 *        them is skipped and counted rather than aborting the lot or being
 *        stored. Override keys this build does not know are dropped quietly: a
 *        backup from a newer Vocs is expected to carry some.
 * WHERE: Called by the BackupPanel in Settings; the file itself is picked and
 *        written through ipc/commands/files.rs.
 */

use std::collections::{HashMap, HashSet};

use tauri::State;

use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::ipc::commands::dictionary::check_entry_text;
use crate::ipc::context::AppState;
use crate::ipc::factory::{execute, CommandSpec, Validate};
use crate::registry::{self, CapabilityKey};
use crate::services::{dictionary, profiles};
use crate::telemetry::now_ms;
use crate::types::{
    AppProfile, BackupDictionaryEntry, BackupFile, ImportSummary, BACKUP_FORMAT, BACKUP_VERSION,
};

/// A backup is two short lists. Anything bigger is not one.
const MAX_BACKUP_BYTES: usize = 5 * 1024 * 1024;
const MAX_DICTIONARY_ENTRIES: usize = 10_000;
const MAX_PROFILES: usize = 500;

const EXPORT: CommandSpec = CommandSpec::new("export_backup", CapabilityKey::Settings).reports();

#[tauri::command]
#[specta::specta]
pub async fn export_backup(state: State<'_, AppState>) -> Result<String, AppError> {
    execute(&state, EXPORT, (), |ctx, ()| async move {
        let entries = dictionary::list_entries(ctx.db())?
            .into_iter()
            .map(|entry| BackupDictionaryEntry {
                pattern: entry.pattern,
                replacement: entry.replacement,
                match_kind: entry.match_kind,
                enabled: entry.enabled,
            })
            .collect();
        let backup = BackupFile::new(now_ms(), entries, profiles::list_profiles(ctx.db())?);
        Ok(serde_json::to_string_pretty(&backup)?)
    })
    .await
}

#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct ImportBackupInput {
    pub json: String,
}

impl Validate for ImportBackupInput {
    fn validate(&self) -> Result<(), String> {
        if self.json.trim().is_empty() {
            return Err("That file is empty.".into());
        }
        if self.json.len() > MAX_BACKUP_BYTES {
            return Err("That file is too large to be a Vocs backup.".into());
        }
        Ok(())
    }
}

const IMPORT: CommandSpec = CommandSpec::new("import_backup", CapabilityKey::Settings)
    .exclusive()
    .reports();

#[tauri::command]
#[specta::specta]
pub async fn import_backup(
    state: State<'_, AppState>,
    input: ImportBackupInput,
) -> Result<ImportSummary, AppError> {
    execute(&state, IMPORT, input, |ctx, input| async move {
        let file = parse_backup(&input.json).map_err(AppError::invalid_input)?;
        let (clean, skipped) = sanitise_backup(file);
        apply_backup(ctx.db(), clean, skipped)
    })
    .await
}

/// The part of a backup that survived sanitising.
struct CleanBackup {
    dictionary: Vec<BackupDictionaryEntry>,
    profiles: Vec<AppProfile>,
}

/**
 * WHAT:  Merges a sanitised backup into the database and returns the tally.
 * WHY:   Separate from the command so it can be tested against an in-memory
 *        database. Only adds and updates; never deletes (see the module WHY).
 */
fn apply_backup(db: &Database, clean: CleanBackup, mut summary: ImportSummary) -> AppResult<ImportSummary> {
    let known_entries: HashSet<(String, &'static str)> = dictionary::list_entries(db)?
        .into_iter()
        .map(|entry| (entry.pattern, entry.match_kind.as_str()))
        .collect();
    for entry in clean.dictionary {
        let existed = known_entries.contains(&(entry.pattern.clone(), entry.match_kind.as_str()));
        let id = dictionary::create_entry(db, &entry.pattern, &entry.replacement, entry.match_kind, now_ms())?;
        // create_entry always (re-)enables, so a disabled entry in the backup
        // needs a second write to stay disabled.
        if !entry.enabled {
            dictionary::update_entry(db, &id, &entry.replacement, entry.match_kind, false)?;
        }
        if existed {
            summary.dictionary_updated += 1;
        } else {
            summary.dictionary_added += 1;
        }
    }

    let known_profiles: HashSet<String> =
        profiles::list_profiles(db)?.into_iter().map(|profile| profile.bundle_id).collect();
    for profile in clean.profiles {
        let existed = known_profiles.contains(&profile.bundle_id);
        profiles::upsert_profile(db, &profile)?;
        if existed {
            summary.profiles_updated += 1;
        } else {
            summary.profiles_added += 1;
        }
    }
    Ok(summary)
}

/// Parses and identifies the file. The messages are shown to the user.
fn parse_backup(json: &str) -> Result<BackupFile, String> {
    let file: BackupFile = serde_json::from_str(json)
        .map_err(|_| "That is not a Vocs backup file.".to_string())?;
    if file.format != BACKUP_FORMAT {
        return Err("That is not a Vocs backup file.".into());
    }
    if file.version > BACKUP_VERSION {
        return Err("That backup was made by a newer version of Vocs. Update Vocs and try again.".into());
    }
    if file.dictionary.len() > MAX_DICTIONARY_ENTRIES || file.profiles.len() > MAX_PROFILES {
        return Err("That backup has more entries than Vocs can restore.".into());
    }
    Ok(file)
}

/// Applies the live validation rules to every entry. Returns what is safe to
/// store, with the skipped counts already filled in.
fn sanitise_backup(file: BackupFile) -> (CleanBackup, ImportSummary) {
    let mut summary = ImportSummary::default();

    // Keyed so a duplicate inside the file collapses, last one winning — the
    // same outcome as importing them one after another.
    let mut dictionary: HashMap<(String, &'static str), BackupDictionaryEntry> = HashMap::new();
    let mut order: Vec<(String, &'static str)> = Vec::new();
    for entry in file.dictionary {
        let pattern = entry.pattern.trim().to_string();
        let replacement = entry.replacement.trim().to_string();
        if check_entry_text(&pattern, &replacement).is_err() {
            summary.dictionary_skipped += 1;
            continue;
        }
        let key = (pattern.clone(), entry.match_kind.as_str());
        if dictionary
            .insert(key.clone(), BackupDictionaryEntry { pattern, replacement, ..entry })
            .is_none()
        {
            order.push(key);
        }
    }
    let dictionary = order.into_iter().filter_map(|key| dictionary.remove(&key)).collect();

    let mut kept: HashMap<String, AppProfile> = HashMap::new();
    let mut profile_order: Vec<String> = Vec::new();
    for mut profile in file.profiles {
        let bundle_id = profile.bundle_id.trim().to_string();
        if bundle_id.is_empty() {
            summary.profiles_skipped += 1;
            continue;
        }
        // Unknown keys are dropped, known keys must validate.
        profile.overrides.retain(|key, _| registry::setting_def(key).is_some());
        let valid = profile.overrides.iter().all(|(key, value)| {
            registry::setting_def(key).is_some_and(|def| def.validate_value(value).is_ok())
        });
        if !valid {
            summary.profiles_skipped += 1;
            continue;
        }
        if profile.display_name.trim().is_empty() {
            profile.display_name = bundle_id.clone();
        }
        profile.bundle_id = bundle_id.clone();
        if kept.insert(bundle_id.clone(), profile).is_none() {
            profile_order.push(bundle_id);
        }
    }
    let profiles = profile_order.into_iter().filter_map(|id| kept.remove(&id)).collect();

    (CleanBackup { dictionary, profiles }, summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::keys;
    use crate::types::{MatchKind, SettingValue};

    fn entry(pattern: &str, replacement: &str) -> BackupDictionaryEntry {
        BackupDictionaryEntry {
            pattern: pattern.into(),
            replacement: replacement.into(),
            match_kind: MatchKind::Word,
            enabled: true,
        }
    }

    fn profile(bundle_id: &str, overrides: &[(&str, SettingValue)]) -> AppProfile {
        AppProfile {
            bundle_id: bundle_id.into(),
            display_name: "App".into(),
            overrides: overrides.iter().map(|(k, v)| ((*k).to_string(), v.clone())).collect(),
            enabled: true,
            excluded: false,
        }
    }

    fn file(dictionary: Vec<BackupDictionaryEntry>, profiles: Vec<AppProfile>) -> BackupFile {
        BackupFile::new(0, dictionary, profiles)
    }

    #[test]
    fn a_backup_round_trips_through_its_own_json() {
        let original = file(vec![entry("clod", "Claude")], vec![profile("com.x", &[])]);
        let json = serde_json::to_string_pretty(&original).expect("serialises");
        let parsed = parse_backup(&json).expect("parses");
        assert_eq!(parsed.dictionary, original.dictionary);
        assert_eq!(parsed.profiles.len(), 1);
    }

    #[test]
    fn a_json_file_that_is_not_a_backup_is_refused_by_name() {
        assert!(parse_backup(r#"{"hello":"world"}"#).is_err());
        assert!(parse_backup("not json at all").is_err());
        let wrong_format = r#"{"format":"something-else","version":1,"exported_at":0}"#;
        assert!(parse_backup(wrong_format).is_err());
    }

    #[test]
    fn a_backup_from_a_newer_vocs_is_refused_rather_than_half_applied() {
        let newer = format!(
            r#"{{"format":"{BACKUP_FORMAT}","version":{},"exported_at":0}}"#,
            BACKUP_VERSION + 1
        );
        assert!(parse_backup(&newer).is_err());
    }

    #[test]
    fn missing_lists_mean_empty_lists() {
        let minimal = format!(r#"{{"format":"{BACKUP_FORMAT}","version":1,"exported_at":0}}"#);
        let parsed = parse_backup(&minimal).expect("a hand-trimmed file is still valid");
        assert!(parsed.dictionary.is_empty() && parsed.profiles.is_empty());
    }

    #[test]
    fn unusable_dictionary_entries_are_skipped_and_counted() {
        let (clean, summary) = sanitise_backup(file(
            vec![
                entry("good", "Good"),
                entry("   ", "empty pattern matches everywhere"),
                entry("no replacement", ""),
                entry(&"x".repeat(500), "too long"),
            ],
            vec![],
        ));
        assert_eq!(clean.dictionary.len(), 1);
        assert_eq!(summary.dictionary_skipped, 3);
    }

    #[test]
    fn entries_are_trimmed_and_duplicates_collapse_with_the_last_winning() {
        let (clean, _) = sanitise_backup(file(
            vec![entry(" clod ", "Claude"), entry("clod", "Claude Code")],
            vec![],
        ));
        assert_eq!(clean.dictionary.len(), 1);
        assert_eq!(clean.dictionary[0].pattern, "clod");
        assert_eq!(clean.dictionary[0].replacement, "Claude Code");
    }

    #[test]
    fn a_profile_with_an_out_of_range_value_is_skipped_whole() {
        let bad = profile("com.bad", &[(keys::PASTE_DELAY_MS, SettingValue::Number(99_999.0))]);
        let good = profile("com.good", &[(keys::PASTE_DELAY_MS, SettingValue::Number(100.0))]);
        let (clean, summary) = sanitise_backup(file(vec![], vec![bad, good]));
        assert_eq!(clean.profiles.len(), 1);
        assert_eq!(clean.profiles[0].bundle_id, "com.good");
        assert_eq!(summary.profiles_skipped, 1);
    }

    #[test]
    fn a_profile_naming_a_setting_this_build_lacks_keeps_the_rest() {
        let from_newer = profile(
            "com.x",
            &[
                ("future.setting", SettingValue::Bool(true)),
                (keys::STRIP_FILLERS, SettingValue::Bool(true)),
            ],
        );
        let (clean, summary) = sanitise_backup(file(vec![], vec![from_newer]));
        assert_eq!(summary.profiles_skipped, 0);
        let overrides = &clean.profiles[0].overrides;
        assert!(overrides.contains_key(keys::STRIP_FILLERS));
        assert!(!overrides.contains_key("future.setting"));
    }

    #[test]
    fn a_profile_without_an_app_is_skipped_and_a_blank_name_falls_back() {
        let mut nameless = profile("com.x", &[]);
        nameless.display_name = "  ".into();
        let (clean, summary) = sanitise_backup(file(vec![], vec![profile("  ", &[]), nameless]));
        assert_eq!(summary.profiles_skipped, 1);
        assert_eq!(clean.profiles[0].display_name, "com.x");
    }

    fn restore(db: &Database, backup: BackupFile) -> ImportSummary {
        let (clean, skipped) = sanitise_backup(backup);
        apply_backup(db, clean, skipped).expect("applies")
    }

    #[test]
    fn importing_adds_new_entries_and_updates_existing_ones_without_deleting_any() -> AppResult<()> {
        let db = Database::open_in_memory()?;
        dictionary::create_entry(&db, "local only", "Local Only", MatchKind::Word, 1)?;
        dictionary::create_entry(&db, "clod", "Clod", MatchKind::Word, 1)?;

        let summary = restore(
            &db,
            file(vec![entry("clod", "Claude"), entry("tory", "Tauri")], vec![profile("com.x", &[])]),
        );

        assert_eq!((summary.dictionary_added, summary.dictionary_updated), (1, 1));
        assert_eq!((summary.profiles_added, summary.profiles_updated), (1, 0));
        let stored = dictionary::list_entries(&db)?;
        assert_eq!(stored.len(), 3, "the machine's own entry survives a restore");
        assert!(stored.iter().any(|e| e.pattern == "clod" && e.replacement == "Claude"));
        Ok(())
    }

    #[test]
    fn restoring_the_same_backup_twice_changes_nothing_the_second_time() -> AppResult<()> {
        let db = Database::open_in_memory()?;
        let backup = || file(vec![entry("clod", "Claude")], vec![profile("com.x", &[])]);

        restore(&db, backup());
        let again = restore(&db, backup());

        assert_eq!((again.dictionary_added, again.dictionary_updated), (0, 1));
        assert_eq!((again.profiles_added, again.profiles_updated), (0, 1));
        assert_eq!(dictionary::list_entries(&db)?.len(), 1);
        assert_eq!(profiles::list_profiles(&db)?.len(), 1);
        Ok(())
    }

    #[test]
    fn a_disabled_entry_stays_disabled_after_a_restore() -> AppResult<()> {
        let db = Database::open_in_memory()?;
        let mut off = entry("clod", "Claude");
        off.enabled = false;
        restore(&db, file(vec![off], vec![]));
        assert!(!dictionary::list_entries(&db)?[0].enabled);
        Ok(())
    }

    #[test]
    fn an_export_restores_cleanly_into_an_empty_database() -> AppResult<()> {
        let source = Database::open_in_memory()?;
        dictionary::create_entry(&source, "clod", "Claude", MatchKind::WordCaseSensitive, 1)?;
        profiles::upsert_profile(
            &source,
            &profile("com.apple.Terminal", &[(keys::STRIP_FILLERS, SettingValue::Bool(true))]),
        )?;

        let exported = BackupFile::new(
            0,
            dictionary::list_entries(&source)?
                .into_iter()
                .map(|e| BackupDictionaryEntry {
                    pattern: e.pattern,
                    replacement: e.replacement,
                    match_kind: e.match_kind,
                    enabled: e.enabled,
                })
                .collect(),
            profiles::list_profiles(&source)?,
        );
        let json = serde_json::to_string_pretty(&exported)?;

        let target = Database::open_in_memory()?;
        let summary = restore(&target, parse_backup(&json).map_err(AppError::invalid_input)?);

        assert_eq!((summary.dictionary_added, summary.profiles_added), (1, 1));
        let restored = dictionary::list_entries(&target)?;
        assert_eq!(restored[0].match_kind, MatchKind::WordCaseSensitive);
        assert_eq!(
            profiles::list_profiles(&target)?[0].overrides.get(keys::STRIP_FILLERS),
            Some(&SettingValue::Bool(true))
        );
        Ok(())
    }

    #[test]
    fn an_oversized_backup_is_refused() {
        let many = (0..=MAX_DICTIONARY_ENTRIES).map(|i| entry(&format!("p{i}"), "r")).collect();
        let json = serde_json::to_string(&file(many, vec![])).expect("serialises");
        assert!(parse_backup(&json).is_err());
    }
}
