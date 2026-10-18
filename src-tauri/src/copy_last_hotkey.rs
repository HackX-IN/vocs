/*!
 * SOURCE OF TRUTH KEYWORDS: copy_last_hotkey, COPY_LAST_SHORTCUT, apply_copy_last,
 *   copy_last_binding, global_shortcut, X9
 * WHAT:  The optional global shortcut that copies the most recent transcript to
 *        the clipboard, kept in step with its two settings.
 * WHY:   The tray item already does this, but a failed paste is exactly when
 *        the user's hands are on the keyboard. It is a separate module rather
 *        than a second case in `bootstrap` because dictation owns a
 *        chord-or-modifier-tap pair that assumes it is the only binding;
 *        copy-last is a plain chord, registered independently, so the two can
 *        never tear each other down.
 *
 *        OFF by default. A global shortcut takes a key away from every other
 *        app, and nobody asked for that until they turn it on.
 * WHERE: `bootstrap::setup` calls `apply` once at launch, and the settings
 *        listener calls it again whenever either setting changes.
 */

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::bootstrap::{dictation_binding, to_shortcut};
use crate::error::AppResult;
use crate::ipc::context::AppState;
use crate::registry::{self, keys};
use crate::services;
use crate::types::{HotkeyBinding, SettingValue};

/**
 * SOURCE OF TRUTH KEYWORDS: COPY_LAST_SHORTCUT, registered_copy_last
 * WHAT:  The copy-last shortcut currently registered with the OS.
 * WHY:   Rebinding has to unregister the OLD accelerator, and by the time the
 *        settings listener runs the database already holds the NEW one.
 * WHERE: Written only by `apply`.
 */
static COPY_LAST_SHORTCUT: parking_lot::Mutex<Option<Shortcut>> = parking_lot::Mutex::new(None);

/**
 * WHAT:  Makes the OS agree with the copy-last settings.
 * WHY:   Always releases first, then registers only when the toggle is on.
 *        Reconciling the whole state rather than reacting to which key changed
 *        keeps it correct for a reset or an import, which change several
 *        settings at once. Every refusal is logged and leaves nothing
 *        registered — a shortcut that half-works is worse than none.
 * WHERE: Launch, and the SettingsChanged listener in bootstrap.
 */
pub fn apply(app: &AppHandle) {
    release(app);

    // The shortcut recorder in Settings has every global shortcut released.
    if crate::bootstrap::hotkeys_paused() {
        return;
    }

    let Some(state) = app.try_state::<AppState>() else {
        return;
    };

    if !is_enabled(&state.db) {
        return;
    }

    let binding = copy_last_binding(&state.db);
    if let Err(reason) = usable(&binding, &dictation_binding(&state.db)) {
        tracing::warn!(%reason, "the copy-last shortcut was not registered");
        return;
    }

    match register(app, &binding) {
        Ok(()) => tracing::info!(accelerator = binding.to_accelerator(), "copy-last shortcut registered"),
        Err(err) => tracing::error!(error = %err, "could not register the copy-last shortcut"),
    }
}

pub fn release(app: &AppHandle) {
    let Some(previous) = COPY_LAST_SHORTCUT.lock().take() else {
        return;
    };

    let manager = app.global_shortcut();
    if manager.is_registered(previous) {
        if let Err(err) = manager.unregister(previous) {
            tracing::warn!(error = %err, "could not release the old copy-last shortcut");
        }
    }
}

fn register(app: &AppHandle, binding: &HotkeyBinding) -> AppResult<()> {
    let shortcut = to_shortcut(binding)?;
    let handler_app = app.clone();

    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            // Pressed only: the release edge of the same chord must not copy twice.
            if event.state() == ShortcutState::Pressed {
                crate::tray::copy_last_transcript(&handler_app);
            }
        })
        .map_err(|err| {
            crate::error::AppError::new(
                crate::error::ErrorCode::HotkeyRegistrationFailed,
                "Vocs could not register the copy-last shortcut. Another app may already be using it.",
            )
            .recoverable()
            .with_detail(err)
        })?;

    *COPY_LAST_SHORTCUT.lock() = Some(shortcut);
    Ok(())
}

fn is_enabled(db: &crate::db::Database) -> bool {
    matches!(
        services::settings::get_setting(db, keys::COPY_LAST_ENABLED).ok().flatten(),
        Some(SettingValue::Bool(true))
    )
}

/// The user's binding if they have set one, otherwise the registry default.
fn copy_last_binding(db: &crate::db::Database) -> HotkeyBinding {
    if let Some(SettingValue::Hotkey(binding)) =
        services::settings::get_setting(db, keys::COPY_LAST_HOTKEY).ok().flatten()
    {
        return binding;
    }

    match registry::setting_def(keys::COPY_LAST_HOTKEY).map(|def| &def.default) {
        Some(SettingValue::Hotkey(binding)) => binding.clone(),
        _ => HotkeyBinding {
            modifiers: vec![crate::types::KeyModifier::Control, crate::types::KeyModifier::Option],
            key: "KeyC".to_string(),
        },
    }
}

/**
 * WHAT:  Whether `binding` may be registered as the copy-last shortcut.
 * WHY:   A bare modifier needs the event tap that dictation owns, and sharing a
 *        chord with dictation would make one press both start a recording and
 *        copy — the user would hear the dictation start sound and lose their
 *        clipboard. Both are refused with a reason a log line can carry.
 * WHERE: `apply`.
 */
fn usable(binding: &HotkeyBinding, dictation: &HotkeyBinding) -> Result<(), String> {
    binding.bindable()?;

    if binding.is_modifier_only() {
        return Err("a modifier on its own cannot be a copy-last shortcut".into());
    }
    if binding.to_accelerator() == dictation.to_accelerator() {
        return Err("it is the same shortcut as dictation".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::KeyModifier;

    fn chord(modifiers: Vec<KeyModifier>, key: &str) -> HotkeyBinding {
        HotkeyBinding { modifiers, key: key.to_string() }
    }

    #[test]
    fn a_chord_that_differs_from_dictation_is_usable() {
        let copy = chord(vec![KeyModifier::Control, KeyModifier::Option], "KeyC");
        let dictation = chord(vec![KeyModifier::Option], "Space");
        assert!(usable(&copy, &dictation).is_ok());
    }

    #[test]
    fn the_dictation_chord_is_refused() {
        let same = chord(vec![KeyModifier::Option], "Space");
        assert!(usable(&same, &same.clone()).is_err());
    }

    #[test]
    fn a_bare_modifier_is_refused() {
        let modifier = chord(vec![KeyModifier::Option], "");
        let dictation = chord(vec![KeyModifier::Command], "Space");
        assert!(usable(&modifier, &dictation).is_err());
    }

    #[test]
    fn it_is_off_until_the_toggle_is_written() -> AppResult<()> {
        let db = crate::db::Database::open_in_memory()?;
        assert!(!is_enabled(&db), "a global shortcut must be opt-in");

        services::settings::set_setting(&db, keys::COPY_LAST_ENABLED, &SettingValue::Bool(true), 1)?;
        assert!(is_enabled(&db));
        Ok(())
    }

    #[test]
    fn the_default_binding_is_registrable_and_distinct_from_dictation() -> AppResult<()> {
        let db = crate::db::Database::open_in_memory()?;
        let copy = copy_last_binding(&db);

        assert!(to_shortcut(&copy).is_ok());
        assert!(usable(&copy, &dictation_binding(&db)).is_ok());
        Ok(())
    }
}
