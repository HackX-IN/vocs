/*!
 * SOURCE OF TRUTH KEYWORDS: list_history, get_history_entry,
 *   delete_history_entry, clear_history, purge_history, ListHistoryInput
 * WHAT:  Reading and pruning the local transcript history.
 * WHY:   Searching and filtering are part of listing — one command, one filter,
 *        one OFFSET — so a filtered or searched list scrolls and pages exactly
 *        like the plain one. Retention is applied here rather than in the service, because
 *        "N days ago" is a business rule that needs the user's setting.
 * WHERE: Consumed by the History view.
 */

use tauri::State;

use crate::error::{AppError, ErrorCode};
use crate::ipc::context::AppState;
use crate::ipc::factory::{execute, CommandSpec, Validate};
use crate::registry::CapabilityKey;
use crate::services::sessions;
use crate::telemetry::now_ms;
use crate::types::numeric::TsNumber;
use crate::pipeline::diff::word_diff;
use crate::types::{DiffSegment, HistoryFilter, SessionId, SessionSummary};

/// Bounded so a caller cannot ask for the whole table and stall the UI.
const MAX_PAGE: i64 = 500;

#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct ListHistoryInput {
    #[serde(default)]
    pub filter: HistoryFilter,
    #[specta(type = TsNumber)]
    pub limit: i64,
    #[specta(type = TsNumber)]
    pub offset: i64,
}

impl Validate for ListHistoryInput {
    fn validate(&self) -> Result<(), String> {
        if self.limit <= 0 || self.limit > MAX_PAGE {
            return Err(format!("Limit must be between 1 and {MAX_PAGE}."));
        }
        if self.offset < 0 {
            return Err("Offset cannot be negative.".into());
        }
        Ok(())
    }
}

const LIST: CommandSpec = CommandSpec::new("list_history", CapabilityKey::History);

#[tauri::command]
#[specta::specta]
pub async fn list_history(
    state: State<'_, AppState>,
    input: ListHistoryInput,
) -> Result<Vec<SessionSummary>, AppError> {
    execute(&state, LIST, input, |ctx, input| async move {
        sessions::list_sessions(ctx.db(), &input.filter, input.limit, input.offset)
    })
    .await
}

/**
 * SOURCE OF TRUTH KEYWORDS: last_transcript_text, copy_last_transcript
 * WHAT:  The text of the most recent session that heard anything, honouring a
 *        "use original" choice. None when nothing has ever been dictated.
 * WHY:   The way back from a paste that went nowhere. Clipboard-only delivery
 *        and a focused app that swallowed the paste both leave the words
 *        nowhere the user can reach once they have copied something else; the
 *        words are still in History, and this is the one-step route to them.
 *        A function rather than an IPC command because its only caller is the
 *        menu bar, which has no webview to invoke one.
 * WHERE: tray.rs, "Copy last transcript".
 */
pub fn last_transcript_text(db: &crate::db::Database) -> crate::error::AppResult<Option<String>> {
    Ok(sessions::latest_with_text(db)?.and_then(|session| session.effective_text().map(str::to_owned)))
}

#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct SetRevertedInput {
    pub id: SessionId,
    pub reverted: bool,
}

impl Validate for SetRevertedInput {
    fn validate(&self) -> Result<(), String> {
        if self.id.as_str().trim().is_empty() {
            return Err("A session id is required.".into());
        }
        Ok(())
    }
}

const SET_REVERTED: CommandSpec =
    CommandSpec::new("set_history_reverted", CapabilityKey::History).exclusive();

/**
 * WHAT:  Records that the user chose a transcript's original wording (or
 *        changed their mind and went back to the fix).
 * WHY:   A flag, not an overwrite — see migration 003. Refuses a session that
 *        does not exist rather than reporting success for a write that changed
 *        nothing.
 * WHERE: The "Use original" button in the History detail panel.
 */
#[tauri::command]
#[specta::specta]
pub async fn set_history_reverted(
    state: State<'_, AppState>,
    input: SetRevertedInput,
) -> Result<(), AppError> {
    execute(&state, SET_REVERTED, input, |ctx, input| async move {
        if sessions::set_reverted(ctx.db(), &input.id, input.reverted)? {
            Ok(())
        } else {
            Err(AppError::new(ErrorCode::SessionNotFound, "That transcript no longer exists."))
        }
    })
    .await
}

const DIFF: CommandSpec = CommandSpec::new("diff_history_entry", CapabilityKey::History);

/**
 * WHAT:  Word-level said-vs-fixed comparison for one session.
 * WHY:   Computed on request, for the one row being looked at, rather than for
 *        every row of a page: 200 diffs to render a list that shows one.
 * WHERE: The History detail panel.
 */
#[tauri::command]
#[specta::specta]
pub async fn diff_history_entry(
    state: State<'_, AppState>,
    input: SessionIdInput,
) -> Result<Vec<DiffSegment>, AppError> {
    execute(&state, DIFF, input, |ctx, input| async move {
        let session = sessions::get_session(ctx.db(), &input.id)?
            .ok_or_else(|| AppError::new(ErrorCode::SessionNotFound, "That transcript no longer exists."))?;
        Ok(match (session.raw_text.as_deref(), session.final_text.as_deref()) {
            (Some(raw), Some(fixed)) => word_diff(raw, fixed),
            // Nothing to compare against; the panel shows the one text it has.
            _ => Vec::new(),
        })
    })
    .await
}

#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct SessionIdInput {
    pub id: SessionId,
}

impl Validate for SessionIdInput {
    fn validate(&self) -> Result<(), String> {
        if self.id.as_str().trim().is_empty() {
            return Err("A session id is required.".into());
        }
        Ok(())
    }
}

/**
 * WHAT:  One session by id.
 * WHY:   Returns the summary or a NotFound error, rather than an Option.
 *        Two reasons, and the second is the load-bearing one:
 *          - Asking for an id that does not exist is a real error the UI can
 *            already render, because AppError carries its own presentation.
 *          - specta INLINES `Option<SessionSummary>` in return position while
 *            it references `Vec<SessionSummary>`, so an Option here generated a
 *            second anonymous copy of the shape in TypeScript that had to be
 *            kept in step with the real one by hand. Naming the success type
 *            keeps one definition.
 * WHERE: Opening a single transcript from History.
 */
const GET: CommandSpec = CommandSpec::new("get_history_entry", CapabilityKey::History);

#[tauri::command]
#[specta::specta]
pub async fn get_history_entry(
    state: State<'_, AppState>,
    input: SessionIdInput,
) -> Result<SessionSummary, AppError> {
    execute(&state, GET, input, |ctx, input| async move {
        sessions::get_session(ctx.db(), &input.id)?
            .ok_or_else(|| AppError::not_found("That transcript"))
    })
    .await
}

/// A title is a label, not a document; the cap keeps a paste-gone-wrong out of
/// the row layout and the search index.
const MAX_TITLE_CHARS: usize = 200;
/// Generous — a long dictation is thousands of words — but bounded, so one
/// write cannot be a gigabyte.
const MAX_TEXT_CHARS: usize = 200_000;

#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct UpdateHistoryInput {
    pub id: SessionId,
    /// None or blank clears the title.
    pub title: Option<String>,
    pub text: String,
}

impl Validate for UpdateHistoryInput {
    fn validate(&self) -> Result<(), String> {
        if self.id.as_str().trim().is_empty() {
            return Err("A session id is required.".into());
        }
        if self.title.as_deref().is_some_and(|title| title.chars().count() > MAX_TITLE_CHARS) {
            return Err(format!("A title can be at most {MAX_TITLE_CHARS} characters."));
        }
        if self.text.chars().count() > MAX_TEXT_CHARS {
            return Err(format!("A dictation can be at most {MAX_TEXT_CHARS} characters."));
        }
        Ok(())
    }
}

const UPDATE: CommandSpec = CommandSpec::new("update_history_entry", CapabilityKey::History).exclusive();

/**
 * WHAT:  Saves a dictation's title and hand-edited text and returns the row.
 * WHY:   Exclusive, so two saves of the same row cannot interleave and the
 *        later keystroke always wins. Returns the updated summary rather than
 *        unit so the editor patches its list from the one authoritative copy
 *        (the word count is recomputed server-side) instead of guessing it.
 * WHERE: The dictation editor's autosave.
 */
#[tauri::command]
#[specta::specta]
pub async fn update_history_entry(
    state: State<'_, AppState>,
    input: UpdateHistoryInput,
) -> Result<SessionSummary, AppError> {
    execute(&state, UPDATE, input, |ctx, input| async move {
        sessions::update_session_text(ctx.db(), &input.id, input.title.as_deref(), &input.text, now_ms())?
            .ok_or_else(|| AppError::not_found("That transcript"))
    })
    .await
}

const DELETE: CommandSpec =
    CommandSpec::new("delete_history_entry", CapabilityKey::History).exclusive();

#[tauri::command]
#[specta::specta]
pub async fn delete_history_entry(
    state: State<'_, AppState>,
    input: SessionIdInput,
) -> Result<(), AppError> {
    execute(&state, DELETE, input, |ctx, input| async move {
        sessions::delete_session(ctx.db(), &input.id)
    })
    .await
}

const CLEAR: CommandSpec = CommandSpec::new("clear_history", CapabilityKey::History).exclusive();

/// The privacy escape hatch. Deletes everything, immediately, with no tombstone.
#[tauri::command]
#[specta::specta]
pub async fn clear_history(state: State<'_, AppState>) -> Result<u32, AppError> {
    execute(&state, CLEAR, (), |ctx, ()| async move {
        Ok(sessions::delete_all_sessions(ctx.db())? as u32)
    })
    .await
}

#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct PurgeHistoryInput {
    /// 0 means keep everything forever.
    #[specta(type = TsNumber)]
    pub retention_days: i64,
}

impl Validate for PurgeHistoryInput {
    fn validate(&self) -> Result<(), String> {
        if self.retention_days < 0 {
            return Err("Retention cannot be negative.".into());
        }
        Ok(())
    }
}

const PURGE: CommandSpec = CommandSpec::new("purge_history", CapabilityKey::History).exclusive();

#[tauri::command]
#[specta::specta]
pub async fn purge_history(
    state: State<'_, AppState>,
    input: PurgeHistoryInput,
) -> Result<u32, AppError> {
    execute(&state, PURGE, input, |ctx, input| async move {
        if input.retention_days == 0 {
            return Ok(0);
        }
        let cutoff = now_ms() - input.retention_days * 24 * 60 * 60 * 1000;
        Ok(sessions::purge_older_than(ctx.db(), cutoff)? as u32)
    })
    .await
}

/**
 * SOURCE OF TRUTH KEYWORDS: export_history, ExportHistoryInput, ExportFormat,
 *   to_markdown, to_plaintext
 * WHAT:  Serialises history to JSON, Markdown or plain text and returns the
 *        content for the caller to save.
 * WHY:   It is the user's data and it stays that way — no lock-in was a stated
 *        non-negotiable, and an app that can only show you your transcripts has
 *        quietly become the only place they exist.
 *
 *        Returns the CONTENT rather than writing a file, so the save dialog
 *        stays where the user is and Rust never picks a path on their behalf.
 * WHERE: The export action in History.
 */
#[derive(Debug, Clone, Copy, serde::Deserialize, specta::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExportFormat {
    Json,
    Markdown,
    PlainText,
}

#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct ExportHistoryInput {
    pub format: ExportFormat,
}

impl Validate for ExportHistoryInput {
    fn validate(&self) -> Result<(), String> {
        Ok(())
    }
}

const EXPORT: CommandSpec = CommandSpec::new("export_history", CapabilityKey::History);

#[tauri::command]
#[specta::specta]
pub async fn export_history(
    state: State<'_, AppState>,
    input: ExportHistoryInput,
) -> Result<String, AppError> {
    execute(&state, EXPORT, input, |ctx, input| async move {
        // Exports everything, not a page. A partial export that looks complete
        // is worse than no export at all.
        let total = sessions::count_sessions(ctx.db())?;
        let all = sessions::list_sessions(ctx.db(), &crate::types::HistoryFilter::default(), total.max(1), 0)?;

        Ok(match input.format {
            ExportFormat::Json => serde_json::to_string_pretty(&all)?,
            ExportFormat::Markdown => to_markdown(&all),
            ExportFormat::PlainText => to_plaintext(&all),
        })
    })
    .await
}

fn to_markdown(sessions: &[SessionSummary]) -> String {
    let mut out = String::from("# Vocs history\n\n");
    for session in sessions {
        out.push_str(&format!(
            "## {}\n\n_{} · {} words_\n\n{}\n\n",
            format_timestamp(session.started_at_ms),
            session.language.as_deref().unwrap_or("unknown"),
            session.word_count.unwrap_or(0),
            session.effective_text().unwrap_or("(no transcript)")
        ));
    }
    out
}

fn to_plaintext(sessions: &[SessionSummary]) -> String {
    sessions
        .iter()
        .filter_map(SessionSummary::effective_text)
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// ISO-8601 in UTC. Deliberately not localised: an export is a data file, and a
/// locale-dependent timestamp in one is a problem for whoever reads it later.
fn format_timestamp(epoch_ms: i64) -> String {
    let secs = epoch_ms / 1000;
    let days = secs.div_euclid(86_400);
    let time_of_day = secs.rem_euclid(86_400);
    format!(
        "{} {:02}:{:02}:{:02} UTC",
        crate::ipc::commands::stats::civil_from_days_public(days),
        time_of_day / 3600,
        (time_of_day % 3600) / 60,
        time_of_day % 60
    )
}

#[cfg(test)]
mod export_tests {
    use super::*;
    use crate::types::{DeliveryKind, SessionOutcome};

    fn summary(text: &str) -> SessionSummary {
        SessionSummary {
            id: SessionId("s1".into()),
            started_at_ms: 1_700_000_000_000,
            ended_at_ms: Some(1_700_000_005_000),
            outcome: SessionOutcome::Delivered,
            duration_ms: Some(5000),
            language: Some("en".into()),
            engine_id: "whisper".into(),
            model_id: "turbo".into(),
            raw_text: Some(text.into()),
            final_text: Some(text.into()),
            word_count: Some(text.split_whitespace().count() as i64),
            app_bundle_id: None,
            delivery: DeliveryKind::Pasted,
            error_code: None,
            error_message: None,
            reverted: false,
            title: None,
            edited_at_ms: None,
        }
    }

    #[test]
    fn plaintext_contains_only_the_transcripts() {
        let out = to_plaintext(&[summary("hello there"), summary("second one")]);
        assert_eq!(out, "hello there\n\nsecond one");
    }

    #[test]
    fn markdown_carries_the_metadata_a_transcript_needs_to_be_useful() {
        let out = to_markdown(&[summary("hello there")]);
        assert!(out.contains("hello there"));
        assert!(out.contains("2 words"));
        assert!(out.contains("en"));
        assert!(out.contains("UTC"));
    }

    #[test]
    fn a_reverted_session_exports_the_original_and_a_failed_one_exports_what_was_heard() {
        let mut reverted = summary("Hello there.");
        reverted.raw_text = Some("um hello there".into());
        reverted.reverted = true;
        assert_eq!(to_plaintext(&[reverted]), "um hello there");

        let mut recovered = summary("x");
        recovered.final_text = None;
        recovered.raw_text = Some("what the model heard".into());
        assert_eq!(to_plaintext(&[recovered]), "what the model heard");
    }

    #[test]
    fn a_session_with_no_transcript_does_not_break_an_export() {
        let mut empty = summary("x");
        empty.final_text = None;
        empty.raw_text = None;
        assert!(to_markdown(&[empty.clone()]).contains("(no transcript)"));
        assert_eq!(to_plaintext(&[empty]), "");
    }
}
