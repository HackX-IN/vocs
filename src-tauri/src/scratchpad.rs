/*!
 * SOURCE OF TRUTH KEYWORDS: scratchpad, toggle, show_scratchpad, hide_scratchpad,
 *   insert_into_previous_app, FOCUS_SETTLE, dictation_canvas, PadRoutedInjector
 * WHAT:  Opens and closes the floating dictation scratchpad window, and sends
 *        what was written in it to the app the person came from.
 * WHY:   The scratchpad is where a ramble goes BEFORE it reaches an app. It is a
 *        window of our own, so dictating into it needs no new hotkey. The
 *        words themselves are handed to the pad as an event by PadRoutedInjector,
 *        below, because a synthetic paste into our own window is lost whenever
 *        its text field does not hold keyboard focus.
 *
 *        Handing the result onward is the part that needs care. Vocs is an
 *        accessory app, so hiding the pad does not by itself put the previous app
 *        back in front — and a paste sent before that happens lands in the pad
 *        again. The app is hidden as well when the pad is the only window it has
 *        open (so macOS returns to what was frontmost before), then a short beat
 *        is allowed for focus to settle before the paste goes out. That beat is a
 *        starting value to measure on a real machine, exactly like the paste
 *        delays in docs/03 §9; it is not something this environment can verify.
 *
 *        Every window operation hops to the main thread, as show_dashboard does:
 *        AppKit traps if a window is ordered from anywhere else.
 * WHERE: `toggle` is called by the extra-hotkeys module; `show` and `hide` by the
 *        IPC commands in ipc/commands/scratchpad.rs, which also call `insert`.
 */

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::bootstrap::{DASHBOARD_WINDOW, ONBOARDING_WINDOW, SCRATCHPAD_WINDOW};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::ipc::context::AppState;
use crate::ipc::events::ScratchpadDictation;
use crate::ports::injector::{FrontmostApp, InjectionOutcome, TextInjector};
use crate::ports::InjectionRequest;
use crate::session::SessionSettings;
use crate::types::DeliveryKind;

/// How long to let focus return to the previous app before pasting into it.
const FOCUS_SETTLE: Duration = Duration::from_millis(220);

/// Opens the scratchpad if it is away, puts it away if it is open.
pub fn toggle(app: &AppHandle) {
    let visible = app
        .get_webview_window(SCRATCHPAD_WINDOW)
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);
    if visible {
        hide(app);
    } else {
        show(app);
    }
}

pub fn show(app: &AppHandle) {
    let handle = app.clone();
    if let Err(err) = app.run_on_main_thread(move || show_on_main(&handle)) {
        tracing::error!(error = %err, "could not schedule the scratchpad on the main thread");
    }
}

fn show_on_main(app: &AppHandle) {
    let Some(window) = app.get_webview_window(SCRATCHPAD_WINDOW) else {
        tracing::error!("the scratchpad window is missing");
        return;
    };

    // The same order show_dashboard documents: activate the PROCESS first, or the
    // window of an accessory app arrives behind whatever the person was doing.
    #[cfg(target_os = "macos")]
    if let Err(err) = app.show() {
        tracing::warn!(error = %err, "could not activate the app");
    }
    if let Err(err) = window.show() {
        tracing::warn!(error = %err, "could not show the scratchpad");
    }
    if let Err(err) = window.set_focus() {
        tracing::warn!(error = %err, "could not focus the scratchpad");
    }
}

pub fn hide(app: &AppHandle) {
    let handle = app.clone();
    if let Err(err) = app.run_on_main_thread(move || hide_on_main(&handle)) {
        tracing::error!(error = %err, "could not schedule the scratchpad on the main thread");
    }
}

fn hide_on_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(SCRATCHPAD_WINDOW) {
        if let Err(err) = window.hide() {
            tracing::warn!(error = %err, "could not hide the scratchpad");
        }
    }

    // Hiding the app returns macOS to the application that was frontmost before
    // it. Only when the pad was the sole window: hiding the whole app would also
    // take away a dashboard the person has open.
    #[cfg(target_os = "macos")]
    {
        let another_window_open = [DASHBOARD_WINDOW, ONBOARDING_WINDOW].iter().any(|label| {
            app.get_webview_window(label)
                .and_then(|window| window.is_visible().ok())
                .unwrap_or(false)
        });
        if !another_window_open {
            if let Err(err) = app.hide() {
                tracing::warn!(error = %err, "could not hand focus back");
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (DASHBOARD_WINDOW, ONBOARDING_WINDOW);
}

/**
 * WHAT:  Puts the scratchpad away and pastes `text` into the app that had focus
 *        before it, falling back to the clipboard exactly as a dictation does.
 * WHY:   Uses the injector with the person's own paste settings, so it behaves
 *        like every other delivery: secure input, a missing Accessibility grant
 *        and a clipboard-only preference all degrade the same way. The outcome is
 *        returned so the pad can say whether it pasted or only copied.
 */
pub async fn insert(state: &AppState, text: String) -> AppResult<DeliveryKind> {
    hide(&state.app);
    tokio::time::sleep(FOCUS_SETTLE).await;

    let settings = SessionSettings::load(&state.db);
    let request = InjectionRequest {
        text,
        auto_paste: settings.auto_paste,
        restore_clipboard: settings.restore_clipboard,
        paste_delay_ms: settings.paste_delay_ms,
        clipboard_restore_delay_ms: settings.clipboard_restore_delay_ms,
    };

    // The injector sleeps between the clipboard write and the paste; off the
    // async runtime so that sleep cannot stall other commands (docs/03 §5).
    let injector = Arc::clone(&state.ports.injector);
    let outcome = tokio::task::spawn_blocking(move || injector.deliver(&request))
        .await
        .map_err(|err| {
            AppError::new(
                ErrorCode::InjectionFailed,
                "Vocs could not send that. Your text is still in the scratchpad.",
            )
            .with_detail(err.to_string())
        })??;
    Ok(outcome.delivery)
}

/**
 * SOURCE OF TRUTH KEYWORDS: PadRoutedInjector, routes_to_pad, pad_is_target,
 *   ScratchpadDictation, dictate_into_pad
 * WHAT:  A TextInjector that gives a dictation to the scratchpad directly when
 *        the scratchpad is what the person is using, and otherwise hands the
 *        request to the real injector untouched.
 * WHY:   Dictation reaches the pad as a synthetic ⌘V into Vocs's own window, and
 *        that only lands if the pad's text field holds keyboard focus at that
 *        instant. A click on a mode button, the window being shown again, or the
 *        recording pill taking key status for a moment, and the paste goes
 *        nowhere while the dictation is still reported as pasted. For any other
 *        app a keystroke is the only way in; for our own window it is not, so
 *        the text is sent as an event and the pad inserts it wherever focus is.
 *
 *        A decorator rather than a branch in the delivery code: the domain
 *        should not know a window exists, and the port already says "put this
 *        text where the user was typing". Auto-paste off still means clipboard
 *        only, so it defers to the real injector then. `insert` hides the pad
 *        and waits before delivering, so by the time it asks the pad is no
 *        longer visible and the text goes to the app the person came from.
 * WHERE: Built around MacosInjector in bootstrap::build_ports; called by
 *        session/delivery.rs and by `insert` above.
 */
pub struct PadRoutedInjector {
    app: AppHandle,
    inner: Arc<dyn TextInjector>,
}

impl PadRoutedInjector {
    pub fn new(app: AppHandle, inner: Arc<dyn TextInjector>) -> Self {
        Self { app, inner }
    }

    /// Whether the person is looking at the open scratchpad right now.
    fn pad_is_target(&self) -> bool {
        let Some(pad) = self.app.get_webview_window(SCRATCHPAD_WINDOW) else {
            return false;
        };
        let other_window_focused = [DASHBOARD_WINDOW, ONBOARDING_WINDOW].iter().any(|label| {
            self.app
                .get_webview_window(label)
                .and_then(|window| window.is_focused().ok())
                .unwrap_or(false)
        });
        routes_to_pad(pad.is_visible().unwrap_or(false), other_window_focused, vocs_is_active(&pad))
    }
}

impl TextInjector for PadRoutedInjector {
    fn can_inject(&self) -> bool {
        self.inner.can_inject()
    }

    fn frontmost_app(&self) -> Option<FrontmostApp> {
        self.inner.frontmost_app()
    }

    fn deliver(&self, request: &InjectionRequest) -> AppResult<InjectionOutcome> {
        if request.auto_paste && self.pad_is_target() {
            let sent = ScratchpadDictation { text: request.text.clone() }.emit_to(&self.app, SCRATCHPAD_WINDOW);
            match sent {
                Ok(()) => {
                    return Ok(InjectionOutcome {
                        delivery: DeliveryKind::Pasted,
                        reason: None,
                        clipboard_write_ms: 0.0,
                    });
                }
                // Fall through to the keystroke path rather than lose the words.
                Err(err) => tracing::warn!(error = %err, "could not hand the dictation to the scratchpad"),
            }
        }
        self.inner.deliver(request)
    }
}

/// The scratchpad is the target when it is on screen, Vocs is the app being
/// used, and no other Vocs window with a text field (dashboard, onboarding)
/// has focus instead.
fn routes_to_pad(pad_visible: bool, other_window_focused: bool, vocs_active: bool) -> bool {
    pad_visible && vocs_active && !other_window_focused
}

/// Whether Vocs is the active application. Asked of AppKit rather than the pad
/// window's key state, because the key window can be momentarily another Vocs
/// window (the recording pill) while the app, and the pad, are still the thing
/// in front.
#[cfg(target_os = "macos")]
fn vocs_is_active(_pad: &tauri::WebviewWindow) -> bool {
    objc2_app_kit::NSRunningApplication::currentApplication().isActive()
}

#[cfg(not(target_os = "macos"))]
fn vocs_is_active(pad: &tauri::WebviewWindow) -> bool {
    pad.is_focused().unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::routes_to_pad;

    #[test]
    fn an_open_pad_in_the_active_app_takes_the_dictation() {
        assert!(routes_to_pad(true, false, true));
    }

    #[test]
    fn a_pad_that_is_away_never_does() {
        assert!(!routes_to_pad(false, false, true));
    }

    #[test]
    fn a_floating_pad_does_not_steal_a_dictation_meant_for_another_app() {
        assert!(!routes_to_pad(true, false, false));
    }

    #[test]
    fn a_focused_dashboard_keeps_its_own_dictation() {
        assert!(!routes_to_pad(true, true, true));
    }
}
