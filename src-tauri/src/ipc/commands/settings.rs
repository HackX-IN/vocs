/*!
 * SOURCE OF TRUTH KEYWORDS: get_settings, set_setting, reset_setting,
 *   get_registry, SetSettingInput, effective_settings
 * WHAT:  Reading and writing settings, and handing the registry to the frontend.
 * WHY:   `get_settings` returns the EFFECTIVE values — registry defaults with
 *        stored overrides layered on top — so no caller has to know that an
 *        absent row means "default". That merge is a business rule, which is
 *        why it lives here and not in the service.
 *
 *        Writes are validated against the declared control kind before they are
 *        stored, so a value of the wrong shape is rejected at the boundary
 *        rather than discovered when a control renders empty.
 * WHERE: Consumed by the Settings view, which generates its controls from
 *        `get_registry` and its values from `get_settings`.
 */

use std::collections::HashMap;

use tauri::State;
use tauri_specta::Event;

use crate::error::AppError;
use crate::ipc::context::AppState;
use crate::ipc::factory::{execute, CommandSpec, Validate};
use crate::registry::{self, CapabilityKey, RegistrySnapshot};
use crate::services::settings;
use crate::telemetry::now_ms;
use crate::types::SettingValue;

const GET_ALL: CommandSpec = CommandSpec::new("get_settings", CapabilityKey::Settings);

#[tauri::command]
#[specta::specta]
pub async fn get_settings(
    state: State<'_, AppState>,
) -> Result<HashMap<String, SettingValue>, AppError> {
    execute(&state, GET_ALL, (), |ctx, ()| async move {
        let mut effective = registry::default_settings();
        for (key, value) in settings::get_all_settings(ctx.db())? {
            // A stored value whose type no longer matches its declaration is
            // from an older build. The default is the safe reading — better a
            // reset setting than a control that cannot render.
            match registry::setting_def(&key) {
                Some(def) if value.matches_kind(&def.kind) => {
                    effective.insert(key, value);
                }
                Some(_) => tracing::warn!(key, "stored setting no longer matches its kind"),
                None => tracing::debug!(key, "stored setting is no longer declared"),
            }
        }
        Ok(effective)
    })
    .await
}

#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct SetSettingInput {
    pub key: String,
    pub value: SettingValue,
}

impl Validate for SetSettingInput {
    /**
     * WHY: Both checks are the reason a generated settings UI can be trusted.
     *      An undeclared key would write a row nothing ever reads; a mismatched
     *      kind would store a value no control can render.
     */
    fn validate(&self) -> Result<(), String> {
        let Some(def) = registry::setting_def(&self.key) else {
            return Err(format!("`{}` is not a setting.", self.key));
        };
        def.validate_value(&self.value)
    }
}

const SET: CommandSpec = CommandSpec::new("set_setting", CapabilityKey::Settings).exclusive();

#[tauri::command]
#[specta::specta]
pub async fn set_setting(
    state: State<'_, AppState>,
    input: SetSettingInput,
) -> Result<(), AppError> {
    let app = state.app.clone();
    execute(&state, SET, input, |ctx, input| async move {
        settings::set_setting(ctx.db(), &input.key, &input.value, now_ms())?;
        announce(&app, Some(input.key));
        Ok(())
    })
    .await
}

/**
 * WHAT:  Tells every window a setting changed.
 * WHY:   Three separate documents read settings and none of them share memory,
 *        so without this a hotkey rebound in Settings leaves the pill drawing
 *        the old keycap until it is reloaded. A failure to emit is logged and
 *        swallowed: the write already succeeded, and turning a notification
 *        problem into a save error would be a worse lie.
 * WHERE: Called after every successful settings write.
 */
fn announce(app: &tauri::AppHandle, key: Option<String>) {
    if let Err(err) = (crate::ipc::events::SettingsChanged { key }).emit(app) {
        tracing::warn!(error = %err, "could not announce a settings change");
    }
}

#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct ResetSettingInput {
    pub key: String,
}

impl Validate for ResetSettingInput {
    fn validate(&self) -> Result<(), String> {
        if registry::setting_def(&self.key).is_none() {
            return Err(format!("`{}` is not a setting.", self.key));
        }
        Ok(())
    }
}

const RESET: CommandSpec = CommandSpec::new("reset_setting", CapabilityKey::Settings).exclusive();

#[tauri::command]
#[specta::specta]
pub async fn reset_setting(
    state: State<'_, AppState>,
    input: ResetSettingInput,
) -> Result<(), AppError> {
    let app = state.app.clone();
    execute(&state, RESET, input, |ctx, input| async move {
        settings::reset_setting(ctx.db(), &input.key)?;
        announce(&app, Some(input.key));
        Ok(())
    })
    .await
}

/**
 * WHAT:  The whole capability table, for the frontend to render from.
 * WHY:   The frontend reads the SAME declarations the backend enforces. A
 *        settings section becomes a map over these defs, so adding a setting is
 *        a registry entry and never a new form component.
 * WHERE: Called once by the Settings view and the dashboard shell.
 */
const GET_REGISTRY: CommandSpec = CommandSpec::new("get_registry", CapabilityKey::Settings);

#[tauri::command]
#[specta::specta]
pub async fn get_registry(state: State<'_, AppState>) -> Result<RegistrySnapshot, AppError> {
    execute(&state, GET_REGISTRY, (), |_ctx, ()| async move {
        Ok(registry::snapshot())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{keys, CONTEXT_PROMPT_MAX_LEN};

    fn context_input(text: String) -> SetSettingInput {
        SetSettingInput {
            key: keys::CONTEXT_PROMPT.to_string(),
            value: SettingValue::Text(text),
        }
    }

    #[test]
    fn a_text_setting_at_its_limit_is_accepted_and_one_past_it_is_not() {
        let limit = CONTEXT_PROMPT_MAX_LEN as usize;
        assert!(context_input("a".repeat(limit)).validate().is_ok());
        assert!(context_input("a".repeat(limit + 1)).validate().is_err());
    }

    #[test]
    fn the_limit_counts_characters_not_bytes() {
        // 240 multi-byte characters are well over 240 bytes and still allowed.
        let limit = CONTEXT_PROMPT_MAX_LEN as usize;
        assert!(context_input("é".repeat(limit)).validate().is_ok());
    }
}
