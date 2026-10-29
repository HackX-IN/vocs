/*!
 * SOURCE OF TRUTH KEYWORDS: organize_text, scratchpad_insert, hide_scratchpad,
 *   OrganizeInput, InsertInput, MAX_SCRATCHPAD_CHARS
 * WHAT:  The scratchpad's three IPC commands: turn a ramble into bullets, a task
 *        list or a paragraph; send the result to the app the person came from; and
 *        put the pad away.
 * WHY:   `.reports()` on all three: none of them touches the microphone, which is
 *        what the Dictation capability's preflight guards, and refusing to
 *        organise a note because microphone access is missing would be absurd.
 *        Insert is exclusive so a double-pressed shortcut cannot paste twice.
 *
 *        Organising runs on the blocking pool even though the rules are fast:
 *        the port is where a language model would sit, and a model call must
 *        never be awaited on the async runtime.
 * WHERE: Called by src/app/scratchpad. The window itself is crate::scratchpad.
 */

use std::sync::Arc;

use tauri::State;

use crate::error::{AppError, ErrorCode};
use crate::ipc::context::AppState;
use crate::ipc::factory::{execute, CommandSpec, Validate};
use crate::registry::CapabilityKey;
use crate::session::SessionSettings;
use crate::types::{DeliveryKind, OrganizeMode};

/// A scratchpad is a note, not an archive. Generous enough for an hour of
/// rambling, small enough that organising it is instant.
const MAX_SCRATCHPAD_CHARS: usize = 50_000;

#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct OrganizeInput {
    pub text: String,
    pub mode: OrganizeMode,
}

impl Validate for OrganizeInput {
    fn validate(&self) -> Result<(), String> {
        validate_text(&self.text)
    }
}

#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct InsertInput {
    pub text: String,
}

impl Validate for InsertInput {
    fn validate(&self) -> Result<(), String> {
        validate_text(&self.text)
    }
}

fn validate_text(text: &str) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("There is nothing in the scratchpad yet.".into());
    }
    if text.chars().count() > MAX_SCRATCHPAD_CHARS {
        return Err("That is too much for the scratchpad to handle at once.".into());
    }
    Ok(())
}

const ORGANIZE: CommandSpec = CommandSpec::new("organize_text", CapabilityKey::Dictation).reports();

/**
 * WHAT:  `text` restructured per `mode`.
 * WHY:   The language comes from the person's settings, so a Hindi note is laid
 *        out but never sorted with English cues.
 */
#[tauri::command]
#[specta::specta]
pub async fn organize_text(state: State<'_, AppState>, input: OrganizeInput) -> Result<String, AppError> {
    execute(&state, ORGANIZE, input, |ctx, input| async move {
        let organizer = Arc::clone(&ctx.ports().organizer);
        let language = SessionSettings::load(ctx.db()).language;
        tokio::task::spawn_blocking(move || organizer.organize(&input.text, input.mode, language.as_ref()))
            .await
            .map_err(|err| {
                AppError::new(ErrorCode::Internal, "Organising stopped unexpectedly.").with_detail(err.to_string())
            })?
    })
    .await
}

const INSERT: CommandSpec = CommandSpec::new("scratchpad_insert", CapabilityKey::Dictation).reports().exclusive();

#[tauri::command]
#[specta::specta]
pub async fn scratchpad_insert(state: State<'_, AppState>, input: InsertInput) -> Result<DeliveryKind, AppError> {
    execute(&state, INSERT, input, |ctx, input| async move {
        crate::scratchpad::insert(&ctx.state, input.text).await
    })
    .await
}

const HIDE: CommandSpec = CommandSpec::new("hide_scratchpad", CapabilityKey::Dictation).reports();

#[tauri::command]
#[specta::specta]
pub async fn hide_scratchpad(state: State<'_, AppState>) -> Result<(), AppError> {
    execute(&state, HIDE, (), |ctx, ()| async move {
        crate::scratchpad::hide(&ctx.state.app);
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_note_is_refused_with_a_sentence() {
        assert!(validate_text("  \n ").is_err());
        assert!(validate_text("Call Sam.").is_ok());
    }

    #[test]
    fn the_limit_counts_characters_not_bytes() {
        // 50,000 two-byte characters are within the limit; one more is not.
        assert!(validate_text(&"é".repeat(MAX_SCRATCHPAD_CHARS)).is_ok());
        assert!(validate_text(&"é".repeat(MAX_SCRATCHPAD_CHARS + 1)).is_err());
    }
}
