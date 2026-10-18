/*!
 * SOURCE OF TRUTH KEYWORDS: list_app_profiles, save_app_profile,
 *   delete_app_profile, SaveProfileInput
 * WHAT:  CRUD for per-application setting overrides.
 * WHY:   Validation refuses an override naming a setting the registry does not
 *        declare. Without that check a profile could carry a key nothing ever
 *        reads — which looks like a saved preference that silently does
 *        nothing, and is very hard to tell apart from a bug in the feature it
 *        was supposed to change.
 * WHERE: The per-app section of Settings.
 */

use tauri::State;

use crate::error::AppError;
use crate::ipc::context::AppState;
use crate::ipc::factory::{execute, CommandSpec, Validate};
use crate::registry::{self, CapabilityKey};
use crate::services::profiles;
use crate::types::AppProfile;

const LIST: CommandSpec = CommandSpec::new("list_app_profiles", CapabilityKey::Settings);

#[tauri::command]
#[specta::specta]
pub async fn list_app_profiles(state: State<'_, AppState>) -> Result<Vec<AppProfile>, AppError> {
    execute(&state, LIST, (), |ctx, ()| async move {
        profiles::list_profiles(ctx.db())
    })
    .await
}

#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct SaveProfileInput {
    pub profile: AppProfile,
}

impl Validate for SaveProfileInput {
    fn validate(&self) -> Result<(), String> {
        if self.profile.bundle_id.trim().is_empty() {
            return Err("A profile needs an application.".into());
        }

        for (key, value) in &self.profile.overrides {
            let Some(def) = registry::setting_def(key) else {
                return Err(format!("`{key}` is not a setting."));
            };
            def.validate_value(value)?;
        }
        Ok(())
    }
}

const SAVE: CommandSpec =
    CommandSpec::new("save_app_profile", CapabilityKey::Settings).exclusive();

#[tauri::command]
#[specta::specta]
pub async fn save_app_profile(
    state: State<'_, AppState>,
    input: SaveProfileInput,
) -> Result<(), AppError> {
    execute(&state, SAVE, input, |ctx, input| async move {
        profiles::upsert_profile(ctx.db(), &input.profile)
    })
    .await
}

#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct DeleteProfileInput {
    pub bundle_id: String,
}

impl Validate for DeleteProfileInput {
    fn validate(&self) -> Result<(), String> {
        if self.bundle_id.trim().is_empty() {
            return Err("A profile needs an application.".into());
        }
        Ok(())
    }
}

const DELETE: CommandSpec =
    CommandSpec::new("delete_app_profile", CapabilityKey::Settings).exclusive();

#[tauri::command]
#[specta::specta]
pub async fn delete_app_profile(
    state: State<'_, AppState>,
    input: DeleteProfileInput,
) -> Result<(), AppError> {
    execute(&state, DELETE, input, |ctx, input| async move {
        profiles::delete_profile(ctx.db(), &input.bundle_id)
    })
    .await
}

#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct SuggestToneInput {
    pub bundle_id: String,
}

impl Validate for SuggestToneInput {
    fn validate(&self) -> Result<(), String> {
        if self.bundle_id.trim().is_empty() {
            return Err("Name an application to suggest a style for.".into());
        }
        Ok(())
    }
}

const SUGGEST_TONE: CommandSpec = CommandSpec::new("suggest_tone", CapabilityKey::Settings);

/**
 * SOURCE OF TRUTH KEYWORDS: suggest_tone, tone_preset_suggestion
 * WHAT:  The writing style the registry suggests for an app — the choice id
 *        `enhance.tone` stores — or "natural" when it has no opinion.
 * WHY:   Returns the plain choice string rather than an Option or a `Tone`:
 *        the setting is stored as that string, so the UI can drop the answer
 *        straight into a profile override with no mapping, and "natural" doubles
 *        as "no suggestion" because it is also the default there is nothing to
 *        override to. (specta also inlines an Option in return position, which
 *        would put a second hand-kept copy of the shape in the bindings.)
 * WHERE: Called by the per-app profile editor when a profile is created, so the
 *        obvious preset is offered — never applied without being shown.
 */
#[tauri::command]
#[specta::specta]
pub async fn suggest_tone(state: State<'_, AppState>, input: SuggestToneInput) -> Result<String, AppError> {
    execute(&state, SUGGEST_TONE, input, |_ctx, input| async move {
        Ok(registry::tone::suggested_tone(&input.bundle_id).as_str().to_string())
    })
    .await
}
