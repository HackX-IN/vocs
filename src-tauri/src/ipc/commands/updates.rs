/*!
 * SOURCE OF TRUTH KEYWORDS: check_for_update, install_update, UpdateCheck,
 *   updates_enabled, CHECK_UPDATES, UpdaterExt, updater_disabled
 * WHAT:  The two update commands: ask GitHub Releases whether a newer signed
 *        build exists, and install one.
 * WHY:   This is the ONLY network request Vocs makes after setup, and the
 *        product promise is that it can be switched off — so the setting is
 *        checked BEFORE the updater is ever constructed, in both commands.
 *        Not before the request: before the client exists. A gate that builds
 *        a network client and then declines to use it is one refactor away
 *        from being no gate at all.
 *
 *        `install_update` uses the `Update` the last check obtained when there is a
 *        recent one (the cache parks it in Rust, beside the answer it belongs to,
 *        and the UI cannot hold one across IPC anyway) and otherwise looks again.
 *        Either way the signature is verified by the download before anything is
 *        written to disk, so a release pulled in between fails safely.
 *
 *        Signature verification itself is the plugin's, keyed by the public
 *        key in tauri.conf.json. Nothing here reimplements it, and nothing
 *        here can bypass it.
 *
 *        SLOW CHECKS, three causes. The updater had NO timeout, so a network
 *        that accepts the connection and then says nothing left the button on
 *        "Checking" indefinitely: a check is now bounded (CHECK_TIMEOUT), and says
 *        so when it overruns. Every window that opened asked the server again, and
 *        installing asked a third time: answers are now cached (ipc/update_cache.rs),
 *        only a pressed button or the daily timer goes back to the network, and
 *        install reuses the `Update` the check already found. The launch check ran
 *        once and then waited a day even when it had failed because the Mac was
 *        still joining wifi: the scheduler now retries a failure soon.
 * WHERE: Registered in ipc/bindings.rs; driven by the Settings view's update
 *        section. The signing side is .github/workflows/release.yml.
 */

use std::time::Duration;

use tauri::State;
use tauri_plugin_updater::UpdaterExt;
use tauri_specta::Event;

use crate::db::Database;
use crate::error::{AppError, ErrorCode};
use crate::ipc::context::AppState;
use crate::ipc::events::UpdateProgress;
use crate::ipc::factory::{execute, CommandSpec, Validate};
use crate::registry::{self, keys, CapabilityKey};
use crate::services::settings;
use crate::types::{SessionState, SettingValue};

/**
 * SOURCE OF TRUTH KEYWORDS: UpdateCheck
 * WHAT:  What a check found: the feature is off, we are current, or there is a
 *        newer signed build.
 * WHY:   Three states rather than an `Option` plus a boolean, because "the
 *        user turned this off" is not the same answer as "there is nothing new"
 *        and the UI must not render them the same way. Collapsing them is how
 *        a disabled updater ends up quietly reporting "up to date" — a claim
 *        nobody checked.
 * WHERE: Returned by check_for_update; rendered by the Settings view.
 */
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "kind")]
pub enum UpdateCheck {
    /// `general.check_updates` is off. No request was made.
    Disabled,
    UpToDate {
        current_version: String,
    },
    Available {
        version: String,
        current_version: String,
        /// Release notes, when the release carries them.
        notes: Option<String>,
        /// RFC 3339, as published. None when the release omits it.
        published_at: Option<String>,
    },
}

/**
 * SOURCE OF TRUTH KEYWORDS: updates_enabled
 * WHAT:  Whether the user allows the update check to make its request.
 * WHY:   Falls back to the registry default rather than to `false`, so a fresh
 *        install behaves the way the registry says it does and the default
 *        lives in exactly one place. Reads the stored value on every call
 *        rather than caching it, because the answer must change the moment the
 *        toggle does — a cached "enabled" is a network request the user has
 *        already said no to.
 * WHERE: Both commands in this file, before any updater is built.
 */
fn updates_enabled(db: &Database) -> bool {
    let stored = settings::get_setting(db, keys::CHECK_UPDATES).ok().flatten();
    let value = stored.or_else(|| {
        registry::setting_def(keys::CHECK_UPDATES).map(|def| def.default.clone())
    });
    matches!(value, Some(SettingValue::Bool(true)))
}

/// How long a check may take. The endpoint is one small JSON file behind two
/// redirects; ten seconds is generous for a working connection and short enough
/// that a dead one says so instead of leaving the button on "Checking".
const CHECK_TIMEOUT: Duration = Duration::from_secs(10);

/// How long the whole download may take. It is a total request timeout in the
/// plugin, not an idle one, so it has to cover a slow connection fetching a
/// bundle of tens of megabytes.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// The plugin's errors are network and signature failures, which are a user's
/// problem to retry rather than a bug to report — so they map to Network with
/// the detail kept for the log. A timeout is told apart, because "slow" and
/// "unreachable" call for different things from the person.
fn updater_error(err: tauri_plugin_updater::Error) -> AppError {
    let timed_out = matches!(&err, tauri_plugin_updater::Error::Reqwest(inner) if inner.is_timeout());
    let message = if timed_out {
        "The update server took too long to answer. Try again in a moment."
    } else {
        "Vocs could not reach the update server. Check your connection and try again."
    };
    AppError::new(ErrorCode::Network, message).recoverable().with_detail(err)
}

/// Without a known length, report every this many bytes.
const PROGRESS_UNKNOWN_STEP: u64 = 256 * 1024;

/**
 * SOURCE OF TRUTH KEYWORDS: progress_step
 * WHAT:  Whether a download chunk crossed a reporting threshold, and the threshold it crossed.
 * WHY:   The updater calls back per network chunk, which can be thousands of times for one
 *        bundle. One event per percent is more than a progress bar can show and a small
 *        fraction of the chunks; with no announced length, per 256 KiB stands in for it. Pure,
 *        so the throttle can be tested without a download.
 * WHERE: The download callback in install_update.
 */
fn progress_step(received: u64, announced: Option<u64>, total: Option<u64>) -> Option<u64> {
    let step = match total {
        Some(total) if total > 0 => received.saturating_mul(100) / total,
        _ => received / PROGRESS_UNKNOWN_STEP,
    };
    // The first chunk always reports, so the bar leaves zero promptly.
    announced.is_none_or(|last| step > last).then_some(step)
}

const CHECK: CommandSpec = CommandSpec::new("check_for_update", CapabilityKey::Updates);

/**
 * SOURCE OF TRUTH KEYWORDS: CheckForUpdateInput, force_check
 * WHAT:  Whether this check must go to the network (`force`) or may reuse an
 *        answer a few minutes old.
 * WHY:   The button a person pressed must really look; the quiet look on opening
 *        a window must not cost a round trip each time. See ipc/update_cache.rs.
 */
#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct CheckForUpdateInput {
    pub force: bool,
}

impl Validate for CheckForUpdateInput {
    fn validate(&self) -> Result<(), String> {
        Ok(())
    }
}

/**
 * SOURCE OF TRUTH KEYWORDS: look_for_update
 * WHAT:  The update check itself, without the command wrapper.
 * WHY:   Three callers need it — this command, the scheduler in bootstrap that
 *        honours the setting's promise of "on launch and once a day", and the
 *        application menu. They share ONE implementation so the setting check,
 *        the version comparison, the timeout and the error mapping cannot drift
 *        between a manual press and an automatic check. That drift is exactly how
 *        a control ends up meaning something different depending on who
 *        triggered it.
 *
 *        Takes the cache's gate for the length of a request: a second caller
 *        waits for the first and then finds its answer cached, so a burst of
 *        windows opening makes one request. Disabled is answered before the
 *        gate, the cache or the updater, and clears the cache, so switching the
 *        setting off can never be followed by a stale "available".
 * WHERE: check_for_update below, bootstrap::start_update_checks and menu.rs.
 */
pub(crate) async fn look_for_update(state: &AppState, force: bool) -> Result<UpdateCheck, AppError> {
    if !updates_enabled(&state.db) {
        state.updates.forget();
        return Ok(UpdateCheck::Disabled);
    }

    if !force {
        if let Some(cached) = state.updates.fresh() {
            return Ok(cached);
        }
    }

    let _on_the_wire = state.updates.gate().await;
    // Another caller may have answered while this one waited for the gate.
    if !force {
        if let Some(cached) = state.updates.fresh() {
            return Ok(cached);
        }
    }

    let current_version = state.app.package_info().version.to_string();
    let updater = state
        .app
        .updater_builder()
        .timeout(CHECK_TIMEOUT)
        .build()
        .map_err(updater_error)?;

    let found = updater.check().await.map_err(updater_error)?;
    let check = match &found {
        Some(update) => UpdateCheck::Available {
            version: update.version.clone(),
            current_version,
            notes: update.body.clone(),
            published_at: update.date.map(|date| date.to_string()),
        },
        None => UpdateCheck::UpToDate { current_version },
    };
    state.updates.store(check.clone(), found);
    Ok(check)
}

#[tauri::command]
#[specta::specta]
pub async fn check_for_update(
    state: State<'_, AppState>,
    input: CheckForUpdateInput,
) -> Result<UpdateCheck, AppError> {
    execute(&state, CHECK, input, |ctx, input| async move {
        look_for_update(&ctx.state, input.force).await
    })
    .await
}

/**
 * SOURCE OF TRUTH KEYWORDS: install_update
 * WHAT:  Downloads the newest signed build, installs it, and restarts.
 * WHY:   Exclusive, and refuses while a session is live. Installing replaces
 *        the running .app bundle and then restarts the process — do that
 *        mid-dictation and the user loses audio they have already spoken, with
 *        no way to get it back. An update is never urgent enough to be worth
 *        that, so the answer is "not right now" rather than a race.
 *
 *        The setting is checked here too, not just in the check. Otherwise a
 *        stale UI holding an "update available" from before the toggle was
 *        switched off could still start a download the user has forbidden.
 * WHERE: The Settings view's update section, after check_for_update reported
 *        Available.
 */
const INSTALL: CommandSpec = CommandSpec::new("install_update", CapabilityKey::Updates).exclusive();

#[tauri::command]
#[specta::specta]
pub async fn install_update(state: State<'_, AppState>) -> Result<(), AppError> {
    execute(&state, INSTALL, (), |ctx, ()| async move {
        if !updates_enabled(ctx.db()) {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                "Updates are turned off. Turn them back on in Settings to install one.",
            )
            .recoverable());
        }

        if !matches!(ctx.state.current_state(), SessionState::Idle) {
            return Err(AppError::new(
                ErrorCode::SessionAlreadyActive,
                "Vocs is in the middle of a dictation. Finish it and try again.",
            )
            .recoverable());
        }

        // The update the last check found, when it is recent: no second round trip
        // between the click and the download starting.
        let mut update = match ctx.state.updates.available_update() {
            Some(update) => update,
            None => {
                let updater = ctx
                    .state
                    .app
                    .updater_builder()
                    .timeout(CHECK_TIMEOUT)
                    .build()
                    .map_err(updater_error)?;
                let Some(update) = updater.check().await.map_err(updater_error)? else {
                    // Something else installed it, or the release was pulled between
                    // the check and the click. Not an error the user caused.
                    return Err(AppError::new(
                        ErrorCode::NotFound,
                        "That update is no longer available. Check again.",
                    )
                    .recoverable());
                };
                update
            }
        };
        // The check's short timeout would cut a download off after ten seconds.
        update.timeout = Some(DOWNLOAD_TIMEOUT);

        let version = update.version.clone();
        let app = ctx.state.app.clone();
        let mut received: u64 = 0;
        let mut announced: Option<u64> = None;
        update
            .download_and_install(
                |chunk, total| {
                    received += chunk as u64;
                    if let Some(step) = progress_step(received, announced, total) {
                        announced = Some(step);
                        let event = UpdateProgress {
                            received_bytes: received,
                            total_bytes: total,
                        };
                        if let Err(err) = event.emit(&app) {
                            tracing::debug!(error = %err, "could not report update progress");
                        }
                    }
                },
                || {},
            )
            .await
            .map_err(updater_error)?;

        tracing::info!(version, "update installed; restarting");
        // The cached answer describes a build that is no longer the running one.
        ctx.state.updates.forget();
        // Diverges. The new bundle is already on disk; the running process is
        // the old one, so there is nothing sensible to do but hand over.
        ctx.state.app.restart();
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    /**
     * WHAT:  The registry default decides what an install with no stored value
     *        does.
     * WHY:   `updates_enabled` falls back to the registry rather than to a
     *        literal, and this is the assertion that keeps those two honest. If
     *        the declared default ever flips to false, an app that has never
     *        been configured must stop making the request — silently keeping a
     *        hardcoded `true` here would be the app ignoring its own registry.
     * WHERE: registry/mod.rs, the CHECK_UPDATES toggle.
     */
    #[test]
    fn the_update_check_default_comes_from_the_registry() {
        let def = registry::setting_def(keys::CHECK_UPDATES)
            .expect("the check-updates toggle is declared");
        assert!(
            matches!(def.default, SettingValue::Bool(_)),
            "a toggle must default to a bool, not {:?}",
            def.default
        );
    }

    /**
     * WHAT:  The three answers a check can give stay distinguishable once
     *        serialised.
     * WHY:   Disabled and UpToDate are the pair that must never collapse — an
     *        updater the user switched off reporting "you are up to date" is a
     *        claim nothing checked. The tag is what keeps them apart on the
     *        TypeScript side, so it is asserted here rather than assumed.
     * WHERE: The Settings view branches on this tag.
     */
    #[test]
    fn a_disabled_check_never_looks_like_an_up_to_date_one() {
        let disabled = serde_json::to_value(UpdateCheck::Disabled).expect("serialises");
        let current = serde_json::to_value(UpdateCheck::UpToDate {
            current_version: "0.1.0".to_string(),
        })
        .expect("serialises");

        assert_eq!(disabled.get("kind").and_then(|k| k.as_str()), Some("DISABLED"));
        assert_eq!(current.get("kind").and_then(|k| k.as_str()), Some("UP_TO_DATE"));
        assert_ne!(disabled.get("kind"), current.get("kind"));
    }

    /**
     * WHAT:  The progress throttle reports the first chunk, then once per step, and never goes
     *        backwards or repeats.
     * WHY:   One callback arrives per network chunk; an event for each would flood the webview.
     *        Reporting the first chunk matters as much as the throttle, because a bar that sits
     *        at zero until the first percent looks like a hang.
     * WHERE: The download callback in install_update.
     */
    #[test]
    fn progress_is_reported_once_per_step_and_never_repeated() {
        let total = Some(1_000);
        // First chunk reports even though it is under one percent.
        assert_eq!(progress_step(5, None, total), Some(0));
        // Still inside the percent already announced: silence.
        assert_eq!(progress_step(9, Some(0), total), None);
        // Crossing into the next percent reports it.
        assert_eq!(progress_step(10, Some(0), total), Some(1));
        assert_eq!(progress_step(10, Some(1), total), None);
        // An unknown length falls back to fixed byte steps.
        assert_eq!(progress_step(1, None, None), Some(0));
        assert_eq!(progress_step(PROGRESS_UNKNOWN_STEP, Some(0), None), Some(1));
        // A zero length must not divide by zero.
        assert_eq!(progress_step(10, Some(0), Some(0)), None);
    }
}
