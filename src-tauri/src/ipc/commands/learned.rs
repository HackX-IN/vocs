/*!
 * SOURCE OF TRUTH KEYWORDS: list_learned, set_learned_auto, delete_learned,
 *   clear_learned, LearnedIdInput, SetLearnedAutoInput
 * WHAT:  The list of corrections Vocs has learned, and the controls over it:
 *        turn one on or off, delete one, forget everything.
 * WHY:   Everything Vocs learns is visible and reversible. A feature that
 *        rewrites your words on its own earns trust only if you can see what it
 *        has learned and take any of it back; a switch the user flips is marked
 *        manual so the tallies cannot flip it back.
 * WHERE: Consumed by the Learned corrections card under Settings → Vocabulary.
 */

use tauri::State;

use crate::error::{AppError, ErrorCode};
use crate::ipc::context::AppState;
use crate::ipc::factory::{execute, CommandSpec, Validate};
use crate::registry::CapabilityKey;
use crate::services::learned;
use crate::types::{LearnedCorrection, LearnedId};

const LIST: CommandSpec = CommandSpec::new("list_learned", CapabilityKey::Dictionary);

#[tauri::command]
#[specta::specta]
pub async fn list_learned(state: State<'_, AppState>) -> Result<Vec<LearnedCorrection>, AppError> {
    execute(&state, LIST, (), |ctx, ()| async move { learned::list(ctx.db()) }).await
}

#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct SetLearnedAutoInput {
    pub id: LearnedId,
    pub auto_apply: bool,
}

impl Validate for SetLearnedAutoInput {
    fn validate(&self) -> Result<(), String> {
        Ok(())
    }
}

const SET_AUTO: CommandSpec = CommandSpec::new("set_learned_auto", CapabilityKey::Dictionary).exclusive();

#[tauri::command]
#[specta::specta]
pub async fn set_learned_auto(state: State<'_, AppState>, input: SetLearnedAutoInput) -> Result<(), AppError> {
    execute(&state, SET_AUTO, input, |ctx, input| async move {
        if learned::set_auto(ctx.db(), input.id, input.auto_apply, true)? {
            Ok(())
        } else {
            Err(AppError::new(ErrorCode::NotFound, "That correction no longer exists."))
        }
    })
    .await
}

#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct LearnedIdInput {
    pub id: LearnedId,
}

impl Validate for LearnedIdInput {
    fn validate(&self) -> Result<(), String> {
        Ok(())
    }
}

const DELETE: CommandSpec = CommandSpec::new("delete_learned", CapabilityKey::Dictionary).exclusive();

#[tauri::command]
#[specta::specta]
pub async fn delete_learned(state: State<'_, AppState>, input: LearnedIdInput) -> Result<(), AppError> {
    execute(&state, DELETE, input, |ctx, input| async move { learned::delete(ctx.db(), input.id) }).await
}

const CLEAR: CommandSpec = CommandSpec::new("clear_learned", CapabilityKey::Dictionary).exclusive();

#[tauri::command]
#[specta::specta]
pub async fn clear_learned(state: State<'_, AppState>) -> Result<(), AppError> {
    execute(&state, CLEAR, (), |ctx, ()| async move { learned::clear(ctx.db()) }).await
}
