/*!
 * SOURCE OF TRUTH KEYWORDS: install_menu, app_menu, menu_bar, MENU_SETTINGS,
 *   MENU_TOGGLE_DICTATION, MENU_CHECK_UPDATES, navigate_dashboard, Help
 * WHAT:  The macOS menu bar shown while Vocs is the frontmost app: Vocs, File,
 *        Edit, View, Window and Help, wired to the same actions the dashboard and
 *        the tray offer.
 * WHY:   Tauri's stock menu is a placeholder — no way to open Settings with ⌘,,
 *        nothing that starts a dictation, nothing under Help. A menu bar is where
 *        a Mac user looks first, and the Edit menu is not decoration: WKWebView
 *        only honours ⌘Z/⌘C/⌘V/⌘A in the dictation editor when the menu
 *        carries those items, so without it the editor cannot undo.
 *
 *        Navigation reuses the rail's channel rather than a second mechanism: the
 *        dashboard already listens for `nav-selected` and routes on it, so a menu
 *        item is just another sender. The string is written down here and in
 *        src/lib/window-events.ts and the two must match — a mismatch is silent,
 *        which is why it is a named constant with this note.
 *
 *        "Check for Updates…" answers every outcome with a dialog, unlike the
 *        scheduled check, which stays quiet: this one was asked for, and a menu
 *        item that does nothing visible reads as broken.
 * WHERE: Installed once by bootstrap::setup, after the tray.
 */

use tauri::menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::bootstrap::DASHBOARD_WINDOW;
use crate::error::{AppError, AppResult};
use crate::ipc::context::AppState;
use crate::session::SessionEvent;

/// Must equal NAV_SELECTED in src/lib/window-events.ts.
const NAV_SELECTED_EVENT: &str = "nav-selected";

const REPOSITORY_URL: &str = "https://github.com/HackX-IN/vocs";

const MENU_SETTINGS: &str = "menu.settings";
const MENU_CHECK_UPDATES: &str = "menu.check_updates";
const MENU_TOGGLE_DICTATION: &str = "menu.toggle_dictation";
const MENU_COPY_LAST: &str = "menu.copy_last";
const MENU_GO_DICTATIONS: &str = "menu.go.history";
const MENU_GO_STATS: &str = "menu.go.stats";
const MENU_GO_SETTINGS: &str = "menu.go.settings";
const MENU_GO_SUPPORT: &str = "menu.go.billing";
const MENU_HELP_DOCS: &str = "menu.help.docs";
const MENU_HELP_ISSUE: &str = "menu.help.issue";
const MENU_HELP_LOGS: &str = "menu.help.logs";

/**
 * SOURCE OF TRUTH KEYWORDS: install_menu
 * WHAT:  Builds the menu bar and starts routing its events.
 * WHERE: Called once by bootstrap::setup.
 */
pub fn install_menu(app: &AppHandle) -> AppResult<()> {
    let menu = build(app).map_err(menu_error)?;
    app.set_menu(menu).map_err(menu_error)?;

    app.on_menu_event(|app, event| match event.id().as_ref() {
        MENU_SETTINGS | MENU_GO_SETTINGS => navigate_dashboard(app, "settings"),
        MENU_GO_DICTATIONS => navigate_dashboard(app, "history"),
        MENU_GO_STATS => navigate_dashboard(app, "stats"),
        MENU_GO_SUPPORT => navigate_dashboard(app, "billing"),
        MENU_TOGGLE_DICTATION => toggle_dictation(app),
        MENU_COPY_LAST => crate::tray::copy_last_transcript(app),
        MENU_CHECK_UPDATES => check_for_updates(app),
        MENU_HELP_DOCS => open_url(app, &format!("{REPOSITORY_URL}/tree/main/docs")),
        MENU_HELP_ISSUE => open_url(app, &format!("{REPOSITORY_URL}/issues/new")),
        MENU_HELP_LOGS => reveal_logs(app),
        // Predefined items (copy, paste, hide, quit…) are handled by AppKit.
        _ => {}
    });

    Ok(())
}

fn build(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let about = PredefinedMenuItem::about(
        app,
        Some("About Vocs"),
        Some(AboutMetadata {
            name: Some("Vocs".into()),
            version: Some(app.package_info().version.to_string()),
            website: Some(REPOSITORY_URL.into()),
            ..Default::default()
        }),
    )?;
    let settings = MenuItem::with_id(app, MENU_SETTINGS, "Settings…", true, Some("CmdOrCtrl+,"))?;
    let updates = MenuItem::with_id(app, MENU_CHECK_UPDATES, "Check for Updates…", true, None::<&str>)?;
    let separator = || PredefinedMenuItem::separator(app);

    let vocs = Submenu::with_items(
        app,
        "Vocs",
        true,
        &[
            &about,
            &updates,
            &separator()?,
            &settings,
            &separator()?,
            &PredefinedMenuItem::services(app, None)?,
            &separator()?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &separator()?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;

    let file = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &MenuItem::with_id(app, MENU_TOGGLE_DICTATION, "Start or Stop Dictation", true, None::<&str>)?,
            &MenuItem::with_id(app, MENU_COPY_LAST, "Copy Last Dictation", true, Some("CmdOrCtrl+Shift+C"))?,
            &separator()?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;

    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &separator()?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;

    let view = Submenu::with_items(
        app,
        "View",
        true,
        &[
            &MenuItem::with_id(app, MENU_GO_DICTATIONS, "Dictations", true, Some("CmdOrCtrl+1"))?,
            &MenuItem::with_id(app, MENU_GO_STATS, "Stats", true, Some("CmdOrCtrl+2"))?,
            &MenuItem::with_id(app, MENU_GO_SETTINGS, "Settings", true, Some("CmdOrCtrl+3"))?,
            &MenuItem::with_id(app, MENU_GO_SUPPORT, "Support", true, Some("CmdOrCtrl+4"))?,
            &separator()?,
            &PredefinedMenuItem::fullscreen(app, None)?,
        ],
    )?;

    let window = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, Some("Zoom"))?,
        ],
    )?;

    let help = Submenu::with_items(
        app,
        "Help",
        true,
        &[
            &MenuItem::with_id(app, MENU_HELP_DOCS, "Vocs Documentation", true, None::<&str>)?,
            &MenuItem::with_id(app, MENU_HELP_ISSUE, "Report an Issue…", true, None::<&str>)?,
            &separator()?,
            &MenuItem::with_id(app, MENU_HELP_LOGS, "Show Logs in Finder", true, None::<&str>)?,
        ],
    )?;

    Menu::with_items(app, &[&vocs, &file, &edit, &view, &window, &help])
}

/// Brings the dashboard forward on `route`.
fn navigate_dashboard(app: &AppHandle, route: &str) {
    crate::tray::show_dashboard(app);
    let payload = serde_json::json!({ "route": route });
    if let Err(err) = app.emit_to(DASHBOARD_WINDOW, NAV_SELECTED_EVENT, payload) {
        tracing::warn!(error = %err, route, "could not navigate the dashboard from the menu");
    }
}

/**
 * WHAT:  Starts a dictation, or stops the one in progress.
 * WHY:   Reads whether a session is capturing and sends the matching event
 *        directly, rather than re-using the hotkey handler: that one maps a
 *        PRESS to start-or-stop in toggle mode but only to "start" in
 *        push-to-talk, where a release that never comes would leave the
 *        recording running.
 */
fn toggle_dictation(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let state = state.inner().clone();
    tauri::async_runtime::spawn(async move {
        let event = if state.current_state().is_capturing() {
            SessionEvent::StopRequested
        } else {
            SessionEvent::StartRequested
        };
        state.session.stamp_request();
        if let Err(err) = state.session.send(event).await {
            tracing::warn!(error = %err, "the menu's dictation toggle was not delivered");
        }
    });
}

fn check_for_updates(app: &AppHandle) {
    use crate::ipc::commands::updates::{look_for_update, UpdateCheck};
    use tauri_specta::Event;

    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let state = state.inner().clone();
    let app = app.clone();

    tauri::async_runtime::spawn(async move {
        let message = match look_for_update(&state, true).await {
            Ok(UpdateCheck::Available { version, current_version, notes, .. }) => {
                let event = crate::ipc::events::UpdateAvailable { version, current_version, notes };
                if let Err(err) = event.emit(&app) {
                    tracing::warn!(error = %err, "could not announce the update");
                }
                crate::tray::show_dashboard(&app);
                return;
            }
            Ok(UpdateCheck::UpToDate { current_version }) => {
                format!("Vocs {current_version} is the newest version.")
            }
            Ok(UpdateCheck::Disabled) => {
                "Update checks are turned off. Turn them on in Settings to look for a new version.".to_string()
            }
            Err(err) => {
                tracing::info!(error = %err, "could not check for updates");
                err.message
            }
        };
        app.dialog().message(message).title("Vocs").show(|_| {});
    });
}

fn open_url(app: &AppHandle, url: &str) {
    if let Err(err) = app.opener().open_url(url, None::<&str>) {
        tracing::warn!(error = %err, url, "could not open the link");
    }
}

fn reveal_logs(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if let Err(err) = app.opener().reveal_item_in_dir(&state.paths.logs_dir) {
        tracing::warn!(error = %err, "could not reveal the logs folder");
    }
}

fn menu_error(err: tauri::Error) -> AppError {
    AppError::internal(err).with_detail("building the menu bar")
}
