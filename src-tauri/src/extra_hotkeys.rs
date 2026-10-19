/*!
 * SOURCE OF TRUTH KEYWORDS: extra_hotkeys, ExtraHotkey, EXTRAS, copy_last_shortcut,
 *   speak_shortcut, safe_mode_shortcut, apply, release, global_shortcut, X9
 * WHAT:  The optional global shortcuts beside dictation — copy the last
 *        transcript, speak into a meeting, meeting safe mode — each registered
 *        only while its settings say so, and kept in step with them.
 * WHY:   A table of shortcuts rather than a module per shortcut: copy-last was
 *        the first and the second and third were about to be copies of it. Each
 *        row names its enable switch, its binding setting and what pressing it
 *        does; the registering, releasing, refusing and reconciling is written
 *        once.
 *
 *        Separate from `bootstrap`'s dictation hotkey because dictation owns a
 *        chord-or-modifier-tap pair that assumes it is the only binding; these
 *        are plain chords, registered independently, so they can never tear
 *        dictation down (or each other).
 *
 *        All OFF by default. A global shortcut takes a key away from every other
 *        app, and nobody asked for that until they turn the feature on. A
 *        shortcut is refused — with a reason in the log, and nothing registered —
 *        if it is a bare modifier, or the same chord as dictation or as another
 *        extra: a press that did two things is worse than one that did none.
 * WHERE: `bootstrap::setup` calls `apply` once at launch, and the settings
 *        listener calls it again whenever any shortcut-related setting changes.
 */

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::bootstrap::{dictation_binding, to_shortcut};
use crate::error::AppResult;
use crate::ipc::context::AppState;
use crate::registry::{self, keys};
use crate::services;
use crate::session::SessionEvent;
use crate::types::{HotkeyBinding, KeyModifier, SettingValue};

/// One optional shortcut.
struct ExtraHotkey {
    name: &'static str,
    enabled_key: &'static str,
    binding_key: &'static str,
    fallback: fn() -> HotkeyBinding,
    /// Runs on the key PRESS only — a release of the same chord must not repeat it.
    on_press: fn(&AppHandle),
}

fn chord(key: &str) -> HotkeyBinding {
    HotkeyBinding { modifiers: vec![KeyModifier::Control, KeyModifier::Option], key: key.to_string() }
}

const EXTRAS: &[ExtraHotkey] = &[
    ExtraHotkey {
        name: "copy-last",
        enabled_key: keys::COPY_LAST_ENABLED,
        binding_key: keys::COPY_LAST_HOTKEY,
        fallback: || chord("KeyC"),
        on_press: crate::tray::copy_last_transcript,
    },
    ExtraHotkey {
        name: "speak",
        enabled_key: keys::SPEAK_ENABLED,
        binding_key: keys::SPEAK_HOTKEY,
        fallback: || chord("Space"),
        on_press: start_or_stop_speaking,
    },
    ExtraHotkey {
        name: "meeting safe mode",
        enabled_key: keys::SPEAK_ENABLED,
        binding_key: keys::SPEAK_SAFE_HOTKEY,
        fallback: || chord("KeyM"),
        on_press: toggle_safe_mode,
    },
];

/// The shortcuts currently registered with the OS, one slot per EXTRAS row.
/// Rebinding has to unregister the OLD accelerator, and by the time the settings
/// listener runs the database already holds the NEW one.
static REGISTERED: parking_lot::Mutex<[Option<Shortcut>; EXTRAS.len()]> =
    parking_lot::Mutex::new([None; EXTRAS.len()]);

/**
 * WHAT:  Makes the OS agree with the shortcut settings.
 * WHY:   Always releases everything first, then registers what is wanted.
 *        Reconciling the whole state rather than reacting to which key changed
 *        keeps it correct for a reset or an import, which change several
 *        settings at once. Every refusal is logged and leaves that shortcut
 *        unregistered — one that half-works is worse than none.
 * WHERE: Launch, the shortcut recorder, and the SettingsChanged listener.
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

    let dictation = dictation_binding(&state.db);
    let mut taken = vec![dictation.to_accelerator()];

    for (slot, extra) in EXTRAS.iter().enumerate() {
        if !is_enabled(&state.db, extra.enabled_key) {
            continue;
        }
        let binding = binding_for(&state.db, extra);
        if let Err(reason) = usable(&binding, &taken) {
            tracing::warn!(shortcut = extra.name, %reason, "a shortcut was not registered");
            continue;
        }
        match register(app, &binding, extra) {
            Ok(shortcut) => {
                REGISTERED.lock()[slot] = Some(shortcut);
                taken.push(binding.to_accelerator());
                tracing::info!(shortcut = extra.name, accelerator = binding.to_accelerator(), "shortcut registered");
            }
            Err(err) => tracing::error!(shortcut = extra.name, error = %err, "could not register a shortcut"),
        }
    }
}

pub fn release(app: &AppHandle) {
    let previous: Vec<Shortcut> = REGISTERED.lock().iter_mut().filter_map(Option::take).collect();
    let manager = app.global_shortcut();
    for shortcut in previous {
        if manager.is_registered(shortcut) {
            if let Err(err) = manager.unregister(shortcut) {
                tracing::warn!(error = %err, "could not release an old shortcut");
            }
        }
    }
}

fn register(app: &AppHandle, binding: &HotkeyBinding, extra: &'static ExtraHotkey) -> AppResult<Shortcut> {
    let shortcut = to_shortcut(binding)?;
    let handler_app = app.clone();

    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                (extra.on_press)(&handler_app);
            }
        })
        .map_err(|err| {
            crate::error::AppError::new(
                crate::error::ErrorCode::HotkeyRegistrationFailed,
                format!("Vocs could not register the {} shortcut. Another app may already be using it.", extra.name),
            )
            .recoverable()
            .with_detail(err)
        })?;
    Ok(shortcut)
}

fn is_enabled(db: &crate::db::Database, key: &str) -> bool {
    matches!(services::settings::get_setting(db, key).ok().flatten(), Some(SettingValue::Bool(true)))
}

/// The user's binding if they have set one, otherwise the registry default.
fn binding_for(db: &crate::db::Database, extra: &ExtraHotkey) -> HotkeyBinding {
    if let Some(SettingValue::Hotkey(binding)) = services::settings::get_setting(db, extra.binding_key).ok().flatten() {
        return binding;
    }
    match registry::setting_def(extra.binding_key).map(|def| &def.default) {
        Some(SettingValue::Hotkey(binding)) => binding.clone(),
        _ => (extra.fallback)(),
    }
}

/**
 * WHAT:  Whether `binding` may be registered, given the accelerators already
 *        taken (dictation's, and any extra registered before it).
 * WHY:   A bare modifier needs the event tap that dictation owns, and sharing a
 *        chord with dictation or another extra would make one press do two
 *        things — start a recording AND copy, or speak AND toggle safe mode. Both
 *        are refused with a reason a log line can carry.
 */
fn usable(binding: &HotkeyBinding, taken: &[String]) -> Result<(), String> {
    binding.bindable()?;
    if binding.is_modifier_only() {
        return Err("a modifier on its own cannot be a shortcut here".into());
    }
    if taken.contains(&binding.to_accelerator()) {
        return Err("it is the same shortcut as another one".into());
    }
    Ok(())
}

/**
 * WHAT:  The Speak shortcut: starts a recording whose result is SPOKEN, or stops
 *        the one in progress.
 * WHY:   The "speak next" flag is set BEFORE the start is sent, and the actor
 *        clears it on every start it sees, so a refused start (already
 *        recording, no microphone) can never leave the flag armed to turn the
 *        next ordinary dictation into a spoken one.
 * WHERE: The Speak shortcut, and the menu bar items.
 */
pub fn start_or_stop_speaking(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let state = state.inner().clone();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let event = if state.current_state().is_capturing() {
            SessionEvent::StopRequested
        } else {
            // Voice mode off means this would start an ORDINARY dictation that
            // pastes. A menu item cannot be allowed to do that by accident, so it
            // takes the person to where Voice mode is turned on instead.
            if !crate::session::SessionSettings::load(&state.db).speak_enabled {
                crate::menu::navigate_dashboard(&app, "settings");
                return;
            }
            state.session.arm_speak();
            SessionEvent::StartRequested
        };
        state.session.stamp_request();
        if let Err(err) = state.session.send(event).await {
            tracing::warn!(error = %err, "the speak shortcut's event was not delivered");
        }
    });
}

/// The meeting-safe-mode shortcut: flips the raw-microphone passthrough.
pub fn toggle_safe_mode(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let state = state.inner().clone();
    // Blocking (it opens audio streams), and it must not wait behind anything.
    tauri::async_runtime::spawn_blocking(move || {
        let settings = crate::session::SessionSettings::load(&state.db);
        match state.speak.toggle_safe_mode(&settings) {
            Ok(on) => tracing::info!(on, "meeting safe mode toggled"),
            Err(err) => tracing::warn!(error = %err, "meeting safe mode could not change"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(modifiers: Vec<KeyModifier>, key: &str) -> HotkeyBinding {
        HotkeyBinding { modifiers, key: key.to_string() }
    }

    fn accelerators(bindings: &[&HotkeyBinding]) -> Vec<String> {
        bindings.iter().map(|b| b.to_accelerator()).collect()
    }

    #[test]
    fn a_chord_that_differs_from_the_others_is_usable() {
        let copy = binding(vec![KeyModifier::Control, KeyModifier::Option], "KeyC");
        let dictation = binding(vec![KeyModifier::Option], "Space");
        assert!(usable(&copy, &accelerators(&[&dictation])).is_ok());
    }

    #[test]
    fn the_dictation_chord_is_refused() {
        let same = binding(vec![KeyModifier::Option], "Space");
        assert!(usable(&same, &accelerators(&[&same.clone()])).is_err());
    }

    #[test]
    fn two_extras_cannot_share_a_chord() {
        let first = binding(vec![KeyModifier::Control, KeyModifier::Option], "KeyM");
        assert!(usable(&first, &accelerators(&[&first.clone()])).is_err());
    }

    #[test]
    fn a_bare_modifier_is_refused() {
        let modifier = binding(vec![KeyModifier::Option], "");
        let dictation = binding(vec![KeyModifier::Command], "Space");
        assert!(usable(&modifier, &accelerators(&[&dictation])).is_err());
    }

    #[test]
    fn every_extra_is_off_until_its_toggle_is_written() -> AppResult<()> {
        let db = crate::db::Database::open_in_memory()?;
        for extra in EXTRAS {
            assert!(!is_enabled(&db, extra.enabled_key), "{} must be opt-in", extra.name);
        }
        services::settings::set_setting(&db, keys::COPY_LAST_ENABLED, &SettingValue::Bool(true), 1)?;
        assert!(is_enabled(&db, keys::COPY_LAST_ENABLED));
        Ok(())
    }

    #[test]
    fn the_default_bindings_are_registrable_and_distinct_from_dictation_and_each_other() -> AppResult<()> {
        let db = crate::db::Database::open_in_memory()?;
        let mut taken = vec![dictation_binding(&db).to_accelerator()];
        for extra in EXTRAS {
            let binding = binding_for(&db, extra);
            assert!(to_shortcut(&binding).is_ok(), "{} default is not registrable", extra.name);
            assert!(usable(&binding, &taken).is_ok(), "{} default clashes with another shortcut", extra.name);
            taken.push(binding.to_accelerator());
        }
        Ok(())
    }
}
