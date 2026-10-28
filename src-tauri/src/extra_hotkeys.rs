/*!
 * SOURCE OF TRUTH KEYWORDS: extra_hotkeys, EXTRA_HOTKEYS, ExtraAction,
 *   language_hint_for, apply_extra_hotkeys, pinned_language_hotkey,
 *   scratchpad_hotkey
 * WHAT:  The optional global shortcuts beyond the main dictation key — each
 *        language-pinned dictation hotkey and the scratchpad toggle — kept in
 *        step with their settings.
 * WHY:   Auto-detect is the weak link for anyone who switches languages: it runs
 *        on the first few seconds of a recording, which is exactly when a short
 *        phrase gives it least to go on, and a wrong guess transcribes the whole
 *        dictation in the wrong language. Pinning fixes that and is also faster
 *        (no detection pass), but a single global pin makes the person change a
 *        setting every time they switch. A pin PER HOTKEY moves the choice to the
 *        moment they already make it — which key they press — and costs nothing
 *        when they do not use it.
 *
 *        A separate module from `bootstrap` for the reason copy_last_hotkey is:
 *        the main dictation binding is a chord-or-modifier-tap pair that assumes
 *        it is alone. These are plain chords registered independently, so the
 *        two can never tear each other down.
 *
 *        One table, because they are all the same thing — an opt-in chord that
 *        must not collide with the others — differing only in what a press does.
 *        Registering them together is what lets each be checked against every
 *        shortcut already claimed. OFF by default, like every global shortcut we
 *        add: a new one is a row in EXTRA_HOTKEYS plus its registry entries, not
 *        new code.
 * WHERE: `bootstrap::setup` calls `apply` at launch; the SettingsChanged listener
 *        calls it again when any of these settings or a neighbouring shortcut
 *        changes; `pause_hotkeys` releases and restores it around the Settings
 *        shortcut recorder.
 */

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::bootstrap::{dictation_binding, on_dictation_hotkey, to_shortcut};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::ipc::context::AppState;
use crate::registry::{self, keys};
use crate::services;
use crate::types::{HotkeyBinding, LanguageCode, LanguageHint, SettingValue, AUTO_LANGUAGE};

/**
 * SOURCE OF TRUTH KEYWORDS: ExtraHotkey, ExtraAction, EXTRA_HOTKEYS
 * WHAT:  One optional shortcut: the setting that switches it on, the setting that
 *        holds its keys, and what pressing it does.
 * WHY:   Data, not code: adding a shortcut must not mean copying a function.
 */
struct ExtraHotkey {
    enabled: &'static str,
    hotkey: &'static str,
    action: ExtraAction,
}

enum ExtraAction {
    /// Start or stop a dictation pinned to the language this setting holds.
    Dictate { language: &'static str },
    /// Open the dictation scratchpad, or put it away.
    ToggleScratchpad,
}

const EXTRA_HOTKEYS: [ExtraHotkey; 3] = [
    ExtraHotkey {
        enabled: keys::LANG_HOTKEY_1_ENABLED,
        hotkey: keys::LANG_HOTKEY_1_HOTKEY,
        action: ExtraAction::Dictate { language: keys::LANG_HOTKEY_1_LANGUAGE },
    },
    ExtraHotkey {
        enabled: keys::LANG_HOTKEY_2_ENABLED,
        hotkey: keys::LANG_HOTKEY_2_HOTKEY,
        action: ExtraAction::Dictate { language: keys::LANG_HOTKEY_2_LANGUAGE },
    },
    ExtraHotkey {
        enabled: keys::SCRATCHPAD_ENABLED,
        hotkey: keys::SCRATCHPAD_HOTKEY,
        action: ExtraAction::ToggleScratchpad,
    },
];

/**
 * SOURCE OF TRUTH KEYWORDS: REGISTERED_EXTRA_SHORTCUTS
 * WHAT:  The shortcuts currently registered with the OS, one cell per slot.
 * WHY:   Rebinding has to unregister the OLD accelerator, and by the time the
 *        settings listener runs the database already holds the new one.
 * WHERE: Written only by `apply` and `release`.
 */
static REGISTERED: parking_lot::Mutex<[Option<Shortcut>; EXTRA_HOTKEYS.len()]> =
    parking_lot::Mutex::new([None; EXTRA_HOTKEYS.len()]);

/// Whether a change to `key` (`None` meaning "several at once") can affect these
/// shortcuts directly. Neighbouring shortcuts are the caller's business.
pub fn is_affected_by(key: Option<&str>) -> bool {
    key.is_none_or(|key| {
        EXTRA_HOTKEYS.iter().any(|extra| {
            key == extra.enabled
                || key == extra.hotkey
                || matches!(&extra.action, ExtraAction::Dictate { language } if key == *language)
        })
    })
}

/**
 * WHAT:  Makes the OS agree with the extra-hotkey settings.
 * WHY:   Releases everything first and re-registers from the database, so one
 *        pass is correct for a rebind, a toggle, a reset or an import alike. Each
 *        shortcut is checked against the ones already claimed — dictation,
 *        copy-last and those before it — and a refusal is logged with its
 *        reason and leaves only that shortcut unregistered: one bad binding must
 *        not take the others down.
 * WHERE: Launch, the SettingsChanged listener, and `pause_hotkeys`.
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

    let mut claimed = vec![dictation_binding(&state.db)];
    if crate::copy_last_hotkey::is_enabled(&state.db) {
        claimed.push(crate::copy_last_hotkey::copy_last_binding(&state.db));
    }

    for (index, extra) in EXTRA_HOTKEYS.iter().enumerate() {
        if !is_enabled(&state.db, extra) {
            continue;
        }
        let binding = binding_for(&state.db, extra);
        if let Err(reason) = usable(&binding, &claimed) {
            tracing::warn!(shortcut = index + 1, %reason, "an extra hotkey was not registered");
            continue;
        }
        match register(app, index, &binding) {
            Ok(()) => {
                tracing::info!(shortcut = index + 1, accelerator = binding.to_accelerator(), "extra hotkey registered");
                claimed.push(binding);
            }
            Err(err) => tracing::error!(shortcut = index + 1, error = %err, "could not register an extra hotkey"),
        }
    }
}

/// Unregisters every language hotkey. Idempotent.
pub fn release(app: &AppHandle) {
    let previous: Vec<Shortcut> = REGISTERED.lock().iter_mut().filter_map(Option::take).collect();
    let manager = app.global_shortcut();
    for shortcut in previous {
        if manager.is_registered(shortcut) {
            if let Err(err) = manager.unregister(shortcut) {
                tracing::warn!(error = %err, "could not release an extra hotkey");
            }
        }
    }
}

fn register(app: &AppHandle, index: usize, binding: &HotkeyBinding) -> AppResult<()> {
    let shortcut = to_shortcut(binding)?;
    let handler_app = app.clone();

    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| match &EXTRA_HOTKEYS[index].action {
            ExtraAction::Dictate { language } => {
                // The language is read at the press, not captured at
                // registration, so it can never be stale if the setting changed
                // without a re-register.
                let hint = handler_app
                    .try_state::<AppState>()
                    .map(|state| language_hint_for(&state.db, language))
                    .unwrap_or_default();
                on_dictation_hotkey(&handler_app, event.state(), Some(hint));
            }
            ExtraAction::ToggleScratchpad => {
                // Pressed only: the release edge of the same chord must not toggle twice.
                if event.state() == ShortcutState::Pressed {
                    crate::scratchpad::toggle(&handler_app);
                }
            }
        })
        .map_err(|err| {
            AppError::new(
                ErrorCode::HotkeyRegistrationFailed,
                "Vocs could not register that shortcut. Another app may already be using it.",
            )
            .recoverable()
            .with_detail(err)
        })?;

    REGISTERED.lock()[index] = Some(shortcut);
    Ok(())
}

fn is_enabled(db: &crate::db::Database, extra: &ExtraHotkey) -> bool {
    matches!(
        services::settings::get_setting(db, extra.enabled).ok().flatten(),
        Some(SettingValue::Bool(true))
    )
}

/// The user's binding if they have set one, otherwise the registry default.
fn binding_for(db: &crate::db::Database, extra: &ExtraHotkey) -> HotkeyBinding {
    if let Some(SettingValue::Hotkey(binding)) = services::settings::get_setting(db, extra.hotkey).ok().flatten() {
        return binding;
    }
    match registry::setting_def(extra.hotkey).map(|def| &def.default) {
        Some(SettingValue::Hotkey(binding)) => binding.clone(),
        // Unreachable while the registry declares the key; an empty binding fails
        // `bindable` and is refused with a reason rather than guessed at.
        _ => HotkeyBinding { modifiers: Vec::new(), key: String::new() },
    }
}

/**
 * WHAT:  The language a hotkey is pinned to: a code, or Auto when it is set to
 *        auto-detect (which is also what an unset one means).
 * WHY:   "auto" is a sentinel, not a language code — pinning a language literally
 *        called "auto" would return nothing from the engine.
 */
fn language_hint_for(db: &crate::db::Database, language_key: &str) -> LanguageHint {
    match services::settings::get_setting(db, language_key).ok().flatten() {
        Some(SettingValue::Choice(code)) if code != AUTO_LANGUAGE && !code.trim().is_empty() => {
            LanguageHint::Pinned { language: LanguageCode(code) }
        }
        _ => LanguageHint::Auto,
    }
}

/**
 * WHAT:  Whether `binding` may be registered, given the shortcuts already claimed.
 * WHY:   A bare modifier needs the event tap that dictation owns, and a shared
 *        chord would make one press do two things. Both are refused with a
 *        reason a log line can carry.
 */
fn usable(binding: &HotkeyBinding, claimed: &[HotkeyBinding]) -> Result<(), String> {
    binding.bindable()?;

    if binding.is_modifier_only() {
        return Err("a modifier on its own cannot be an extra hotkey".into());
    }
    if claimed.iter().any(|other| other.to_accelerator() == binding.to_accelerator()) {
        return Err("it is the same shortcut as another Vocs shortcut".into());
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

    fn db() -> AppResult<crate::db::Database> {
        crate::db::Database::open_in_memory()
    }

    #[test]
    fn a_free_chord_is_usable() {
        let mine = chord(vec![KeyModifier::Control, KeyModifier::Option], "Digit1");
        let dictation = chord(vec![KeyModifier::Option], "Space");
        assert!(usable(&mine, &[dictation]).is_ok());
    }

    #[test]
    fn a_chord_another_shortcut_holds_is_refused() {
        let taken = chord(vec![KeyModifier::Control, KeyModifier::Option], "Digit1");
        assert!(usable(&taken, std::slice::from_ref(&taken)).is_err());
    }

    #[test]
    fn a_bare_modifier_is_refused() {
        let modifier = chord(vec![KeyModifier::Option], "");
        assert!(usable(&modifier, &[]).is_err());
    }

    #[test]
    fn it_is_off_until_a_shortcut_is_switched_on() -> AppResult<()> {
        let db = db()?;
        assert!(!is_enabled(&db, &EXTRA_HOTKEYS[0]), "a global shortcut must be opt-in");

        services::settings::set_setting(&db, keys::LANG_HOTKEY_1_ENABLED, &SettingValue::Bool(true), 1)?;
        assert!(is_enabled(&db, &EXTRA_HOTKEYS[0]));
        assert!(!is_enabled(&db, &EXTRA_HOTKEYS[1]), "shortcuts are independent");
        assert!(!is_enabled(&db, &EXTRA_HOTKEYS[2]), "the scratchpad has its own switch");
        Ok(())
    }

    #[test]
    fn a_hotkey_pins_the_language_it_was_given() -> AppResult<()> {
        let db = db()?;
        assert_eq!(language_hint_for(&db, keys::LANG_HOTKEY_1_LANGUAGE), LanguageHint::Auto, "unset means detect");

        services::settings::set_setting(&db, keys::LANG_HOTKEY_1_LANGUAGE, &SettingValue::Choice("hi".into()), 1)?;
        assert_eq!(
            language_hint_for(&db, keys::LANG_HOTKEY_1_LANGUAGE),
            LanguageHint::Pinned { language: LanguageCode("hi".into()) }
        );
        assert_eq!(
            language_hint_for(&db, keys::LANG_HOTKEY_2_LANGUAGE),
            LanguageHint::Auto,
            "the other hotkey is untouched"
        );
        Ok(())
    }

    #[test]
    fn the_auto_sentinel_is_not_a_language_code() -> AppResult<()> {
        let db = db()?;
        services::settings::set_setting(&db, keys::LANG_HOTKEY_2_LANGUAGE, &SettingValue::Choice("auto".into()), 1)?;
        assert_eq!(language_hint_for(&db, keys::LANG_HOTKEY_2_LANGUAGE), LanguageHint::Auto);
        Ok(())
    }

    #[test]
    fn the_default_bindings_are_registrable_and_distinct_from_everything_else() -> AppResult<()> {
        let db = db()?;
        let mut claimed = vec![dictation_binding(&db), crate::copy_last_hotkey::copy_last_binding(&db)];

        for extra in &EXTRA_HOTKEYS {
            let binding = binding_for(&db, extra);
            assert!(to_shortcut(&binding).is_ok());
            assert!(usable(&binding, &claimed).is_ok(), "{binding:?} clashes with a default");
            claimed.push(binding);
        }
        Ok(())
    }

    #[test]
    fn only_its_own_settings_count_as_a_direct_change() {
        assert!(is_affected_by(None), "a reset or import touches everything");
        assert!(is_affected_by(Some(keys::LANG_HOTKEY_2_LANGUAGE)));
        assert!(is_affected_by(Some(keys::SCRATCHPAD_HOTKEY)));
        assert!(!is_affected_by(Some(keys::TONE)));
    }
}
