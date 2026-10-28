/*!
 * SOURCE OF TRUTH KEYWORDS: get_whats_new, mark_whats_new_seen, WhatsNew,
 *   LAST_SEEN_VERSION, whats_new_decision, decide
 * WHAT:  Tells the dashboard whether to show "What's new" for the version that is
 *        running, and records that the person has seen it.
 * WHY:   The answer is Rust's, like every other piece of state here: the running
 *        version and the last version whose notes were seen both live in Rust, so
 *        the dialog cannot disagree with the app about whether it is due. It is
 *        keyed on the VERSION, not on "an update was installed", because that is
 *        what is true however the new build arrived — through the in-app updater,
 *        by dragging a new DMG over the old app, or after a reinstall.
 *
 *        A fresh install has nothing "new": the person has seen no earlier
 *        version, and greeting someone in their first minute with a list of what
 *        changed since a release they never ran is noise. Setup is how that is
 *        recognised (first-run setup not yet complete), and the version is
 *        recorded silently so the NEXT release is the first one announced.
 *
 *        The last-seen version is bookkeeping, not a preference, so it is stored
 *        straight through the settings service and deliberately NOT declared in
 *        the registry: a registry entry would grow a control for it in Settings
 *        and be writable from the frontend through `set_setting`.
 * WHERE: Called by the dashboard's WhatsNewDialog on mount and on dismissal.
 */

use tauri::State;

use crate::error::AppError;
use crate::ipc::context::AppState;
use crate::ipc::factory::{execute, CommandSpec};
use crate::registry::{keys, CapabilityKey};
use crate::services::settings;
use crate::telemetry::now_ms;
use crate::types::SettingValue;

/**
 * SOURCE OF TRUTH KEYWORDS: WhatsNew
 * WHAT:  The version whose notes are due.
 * WHERE: Returned by get_whats_new; the slides themselves are the frontend's copy,
 *        keyed by this version.
 */
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct WhatsNew {
    pub version: String,
}

/// What to do about the notes for `current`.
#[derive(Debug, PartialEq, Eq)]
enum Decision {
    /// Already seen, or nothing to announce.
    Nothing,
    /// Show them.
    Show,
    /// A fresh install: record the version without showing anything.
    RecordSilently,
}

/// Pure, so the rule that matters can be tested without a database.
fn decide(last_seen: Option<&str>, current: &str, setup_complete: bool) -> Decision {
    match last_seen {
        Some(seen) if seen == current => Decision::Nothing,
        Some(_) => Decision::Show,
        // Never recorded: an upgrade from a build that predates this, or a first run.
        None if setup_complete => Decision::Show,
        None => Decision::RecordSilently,
    }
}

fn last_seen(state: &AppState) -> Option<String> {
    match settings::get_setting(&state.db, keys::LAST_SEEN_VERSION).ok().flatten() {
        Some(SettingValue::Text(version)) => Some(version),
        _ => None,
    }
}

fn record(state: &AppState, version: &str) -> Result<(), AppError> {
    settings::set_setting(&state.db, keys::LAST_SEEN_VERSION, &SettingValue::Text(version.to_string()), now_ms())
}

fn setup_complete(state: &AppState) -> bool {
    matches!(
        settings::get_setting(&state.db, keys::ONBOARDING_COMPLETE).ok().flatten(),
        Some(SettingValue::Bool(true))
    )
}

const GET: CommandSpec = CommandSpec::new("get_whats_new", CapabilityKey::Updates).reports();

#[tauri::command]
#[specta::specta]
pub async fn get_whats_new(state: State<'_, AppState>) -> Result<Option<WhatsNew>, AppError> {
    execute(&state, GET, (), |ctx, ()| async move {
        let current = ctx.state.app.package_info().version.to_string();
        let seen = last_seen(&ctx.state);
        match decide(seen.as_deref(), &current, setup_complete(&ctx.state)) {
            Decision::Show => Ok(Some(WhatsNew { version: current })),
            Decision::RecordSilently => {
                record(&ctx.state, &current)?;
                Ok(None)
            }
            Decision::Nothing => Ok(None),
        }
    })
    .await
}

const MARK: CommandSpec = CommandSpec::new("mark_whats_new_seen", CapabilityKey::Updates).reports();

#[tauri::command]
#[specta::specta]
pub async fn mark_whats_new_seen(state: State<'_, AppState>) -> Result<(), AppError> {
    execute(&state, MARK, (), |ctx, ()| async move {
        let current = ctx.state.app.package_info().version.to_string();
        record(&ctx.state, &current)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_notes_are_shown_once_per_version() {
        assert_eq!(decide(Some("1.0.2"), "1.1.0", true), Decision::Show);
        assert_eq!(decide(Some("1.1.0"), "1.1.0", true), Decision::Nothing);
    }

    #[test]
    fn someone_upgrading_from_a_build_that_never_recorded_a_version_is_shown_them() {
        assert_eq!(decide(None, "1.1.0", true), Decision::Show);
    }

    #[test]
    fn a_fresh_install_is_never_greeted_with_what_changed() {
        assert_eq!(decide(None, "1.1.0", false), Decision::RecordSilently);
    }

    #[test]
    fn a_downgrade_is_not_a_reason_to_stay_silent() {
        // Any difference means the notes this build carries have not been seen.
        assert_eq!(decide(Some("1.2.0"), "1.1.0", true), Decision::Show);
    }
}
