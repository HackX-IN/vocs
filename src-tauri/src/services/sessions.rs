/*!
 * SOURCE OF TRUTH KEYWORDS: create_session, finalize_session, get_session,
 *   list_sessions, delete_session, delete_all_sessions,
 *   find_orphans, mark_orphaned, purge_older_than, NewSession, row_to_summary
 * WHAT:  Pure SQLite access to the sessions table. One verb per function.
 * WHY:   No business rules live here — no deciding what an outcome means, no
 *        calling another service, no orchestration. That is what keeps these
 *        functions reusable by the FSM, by crash recovery and by the History
 *        view without any of them inheriting the others' assumptions.
 *
 *        `create_session` is called BEFORE any audio is transcribed. That is
 *        deliberate and it is the whole basis of crash recovery: a row with no
 *        `ended_at` on the next launch is a session the process died during,
 *        and it can still be finished.
 * WHERE: Called by session/machine.rs on every transition, by session/recovery.rs
 *        at startup, and by ipc/commands/history.rs.
 */

use rusqlite::{params, Connection, Row};

use crate::db::Database;
use crate::error::AppResult;
use crate::types::{DeliveryKind, HistoryFilter, HistoryStatus, SessionId, SessionOutcome, SessionSummary};

/**
 * SOURCE OF TRUTH KEYWORDS: NewSession
 * WHAT:  The columns known at the moment a recording starts.
 * WHY:   Separate from SessionSummary because most of that shape does not exist
 *        yet — modelling it as one optional-heavy struct would let a caller
 *        create a row that claims to be finished.
 * WHERE: Built by session/machine.rs on the transition into Recording.
 */
#[derive(Debug, Clone)]
pub struct NewSession {
    pub id: SessionId,
    pub started_at: i64,
    pub engine_id: String,
    pub model_id: String,
    pub app_bundle_id: Option<String>,
}

/// Writes the in-flight row. Outcome starts as `orphaned` so that a crash
/// before finalize leaves an accurate record rather than a false success.
pub fn create_session(db: &Database, session: &NewSession) -> AppResult<()> {
    db.with_connection(|conn| {
        conn.execute(
            "INSERT INTO sessions (id, started_at, outcome, engine_id, model_id, app_bundle_id, delivery)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                session.id.as_str(),
                session.started_at,
                SessionOutcome::Orphaned.as_str(),
                session.engine_id,
                session.model_id,
                session.app_bundle_id,
                DeliveryKind::None.as_str(),
            ],
        )?;
        Ok(())
    })
}

/// The columns that only exist once a session has ended.
#[derive(Debug, Clone)]
pub struct SessionResult {
    pub ended_at: i64,
    pub outcome: SessionOutcome,
    pub duration_ms: i64,
    pub language: Option<String>,
    pub raw_text: Option<String>,
    pub final_text: Option<String>,
    pub word_count: Option<i64>,
    pub delivery: DeliveryKind,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

pub fn finalize_session(
    db: &Database,
    id: &SessionId,
    result: &SessionResult,
) -> AppResult<()> {
    db.with_connection(|conn| {
        conn.execute(
            "UPDATE sessions
                SET ended_at = ?2, outcome = ?3, duration_ms = ?4, language = ?5,
                    raw_text = ?6, final_text = ?7, word_count = ?8,
                    delivery = ?9, error_code = ?10, error_message = ?11
              WHERE id = ?1",
            params![
                id.as_str(),
                result.ended_at,
                result.outcome.as_str(),
                result.duration_ms,
                result.language,
                result.raw_text,
                result.final_text,
                result.word_count,
                result.delivery.as_str(),
                result.error_code,
                result.error_message,
            ],
        )?;
        Ok(())
    })
}

pub fn get_session(db: &Database, id: &SessionId) -> AppResult<Option<SessionSummary>> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(&format!("{SELECT_COLUMNS} WHERE id = ?1"))?;
        let mut rows = stmt.query(params![id.as_str()])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_summary(row)?)),
            None => Ok(None),
        }
    })
}

/**
 * SOURCE OF TRUTH KEYWORDS: list_sessions, HistoryFilter, search_sessions, paged_history
 * WHAT:  One page of history, newest first, narrowed by a filter.
 * WHY:   The text search goes through the FTS index rather than a LIKE scan,
 *        which stops being usable somewhere in the low thousands of rows — and
 *        this table is expected to reach tens of thousands. The phrase is quoted
 *        here so a user's apostrophe is a character to find, not an FTS syntax
 *        error. Search and filters share one query and one OFFSET because the
 *        list pages: a separate search path returned one fixed page and could
 *        not scroll. Cancelled sessions do not exist, so no filter is needed for
 *        them; orphans and failures DO appear, which is the point of history.
 * WHERE: The History view (via list_history), diagnostics, export, and tests.
 */
pub fn list_sessions(
    db: &Database,
    filter: &HistoryFilter,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<SessionSummary>> {
    let mut clauses: Vec<&str> = Vec::new();
    let mut values: Vec<rusqlite::types::Value> = Vec::new();

    if let Some(query) = filter.query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        clauses.push("rowid IN (SELECT rowid FROM sessions_fts WHERE sessions_fts MATCH ?)");
        values.push(format!("\"{}\"", query.replace('"', "\"\"")).into());
    }
    if let Some(since) = filter.since_ms {
        clauses.push("started_at >= ?");
        values.push(since.into());
    }
    match filter.status {
        HistoryStatus::All => {}
        HistoryStatus::Edited => clauses.push("edited_at IS NOT NULL"),
        HistoryStatus::Failed => clauses.push("outcome = 'failed'"),
    }

    let condition = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };
    values.push(limit.into());
    values.push(offset.into());

    db.with_connection(|conn| {
        let mut stmt = conn.prepare(&format!(
            "{SELECT_COLUMNS}{condition} ORDER BY started_at DESC LIMIT ? OFFSET ?"
        ))?;
        let rows = stmt.query_map(rusqlite::params_from_iter(values), row_to_summary)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    })
}

pub fn delete_session(db: &Database, id: &SessionId) -> AppResult<()> {
    db.with_connection(|conn| {
        conn.execute("DELETE FROM sessions WHERE id = ?1", params![id.as_str()])?;
        Ok(())
    })
}

pub fn delete_all_sessions(db: &Database) -> AppResult<u64> {
    db.with_connection(|conn| {
        let removed = conn.execute("DELETE FROM sessions", [])?;
        Ok(removed as u64)
    })
}

/// Sessions that were never finalized — the process died mid-recording.
/// The most recent session that has any words to give back, whatever its
/// outcome: a recovered or failed session still heard something, and "copy the
/// last thing I said" should not skip it. None when nothing has ever been heard.
pub fn latest_with_text(db: &Database) -> AppResult<Option<SessionSummary>> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(&format!(
            "{SELECT_COLUMNS} WHERE length(trim(coalesce(final_text, raw_text, ''))) > 0 \
             ORDER BY started_at DESC LIMIT 1"
        ))?;
        let mut rows = stmt.query_map([], row_to_summary)?;
        rows.next().transpose().map_err(Into::into)
    })
}

/**
 * SOURCE OF TRUTH KEYWORDS: set_grammar_edits, EditState, unsettled_edits,
 *   set_learned_settled, learned_settled
 * WHAT:  Stores the fixes a dictation received, and reads back the sessions
 *        whose fixes have not yet been counted as kept or undone.
 * WHY:   Kept apart from `finalize_session` so the many callers of that (and its
 *        tests) do not each have to say "no edits". The JSON is opaque here: what
 *        an edit means, and when it is settled, is session/learning.rs's business.
 * WHERE: Written by session/delivery.rs; read by session/learning.rs.
 */
pub fn set_grammar_edits(db: &Database, id: &SessionId, edits_json: Option<&str>) -> AppResult<()> {
    db.with_connection(|conn| {
        conn.execute(
            "UPDATE sessions SET grammar_edits = ?2, learned_settled = 0 WHERE id = ?1",
            params![id.as_str(), edits_json],
        )?;
        Ok(())
    })
}

/// What the learning step needs to judge one session's fixes.
#[derive(Debug, Clone)]
pub struct EditState {
    pub id: SessionId,
    pub edits_json: String,
    /// 0 not counted, 1 counted as kept, 2 counted as undone.
    pub settled: i64,
    pub final_text: String,
    pub reverted: bool,
}

fn row_to_edit_state(row: &Row<'_>) -> rusqlite::Result<EditState> {
    Ok(EditState {
        id: SessionId(row.get(0)?),
        edits_json: row.get(1)?,
        settled: row.get(2)?,
        final_text: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
        reverted: row.get(4)?,
    })
}

const EDIT_STATE_COLUMNS: &str =
    "SELECT id, grammar_edits, learned_settled, final_text, reverted FROM sessions";

/// Sessions with fixes that nobody has counted yet and that ended before `cutoff`.
pub fn unsettled_edits(db: &Database, ended_before: i64, limit: i64) -> AppResult<Vec<EditState>> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(&format!(
            "{EDIT_STATE_COLUMNS} WHERE grammar_edits IS NOT NULL AND learned_settled = 0 \
             AND ended_at IS NOT NULL AND ended_at <= ?1 ORDER BY ended_at LIMIT ?2"
        ))?;
        let rows = stmt.query_map(params![ended_before, limit], row_to_edit_state)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    })
}

pub fn get_edit_state(db: &Database, id: &SessionId) -> AppResult<Option<EditState>> {
    db.with_connection(|conn| {
        let mut stmt =
            conn.prepare(&format!("{EDIT_STATE_COLUMNS} WHERE id = ?1 AND grammar_edits IS NOT NULL"))?;
        let mut rows = stmt.query_map(params![id.as_str()], row_to_edit_state)?;
        rows.next().transpose().map_err(Into::into)
    })
}

pub fn set_learned_settled(db: &Database, id: &SessionId, settled: i64) -> AppResult<()> {
    db.with_connection(|conn| {
        conn.execute("UPDATE sessions SET learned_settled = ?2 WHERE id = ?1", params![id.as_str(), settled])?;
        Ok(())
    })
}

/// Records whether the user chose the original wording. Returns false when no
/// such session exists, so the caller can say so instead of pretending.
pub fn set_reverted(db: &Database, id: &SessionId, reverted: bool) -> AppResult<bool> {
    db.with_connection(|conn| {
        let changed = conn.execute(
            "UPDATE sessions SET reverted = ?2 WHERE id = ?1",
            params![id.as_str(), reverted],
        )?;
        Ok(changed > 0)
    })
}

/**
 * SOURCE OF TRUTH KEYWORDS: update_session_text, edit_dictation, manual_edit
 * WHAT:  Saves the user's own title and wording for one dictation, returning the
 *        updated row, or None when no such session exists.
 * WHY:   The edit goes to `final_text`, the column everything reads (row preview,
 *        copy, export, search) and leaves `raw_text` alone — what was SAID is a
 *        record and a person editing the result must not be able to rewrite
 *        it, which is also what keeps the said-vs-fixed comparison meaningful.
 *        `reverted` is cleared: choosing to work on the text means working on
 *        THIS text, and a stale "use original" flag would make copy and export
 *        ignore the very edit just saved. The word count is recomputed here for
 *        the same reason the row stores one — Stats must agree with the text.
 *        A blank title is stored as NULL so "unnamed" has one representation.
 * WHERE: ipc/commands/history.rs::update_history_entry.
 */
pub fn update_session_text(
    db: &Database,
    id: &SessionId,
    title: Option<&str>,
    text: &str,
    now_ms: i64,
) -> AppResult<Option<SessionSummary>> {
    let title = title.map(str::trim).filter(|title| !title.is_empty());
    let word_count = text.split_whitespace().count() as i64;

    let changed = db.with_connection(|conn| {
        Ok(conn.execute(
            "UPDATE sessions
                SET title = ?2, final_text = ?3, word_count = ?4, reverted = 0, edited_at = ?5
              WHERE id = ?1",
            params![id.as_str(), title, text, word_count, now_ms],
        )?)
    })?;

    if changed == 0 {
        return Ok(None);
    }
    get_session(db, id)
}

pub fn find_orphans(db: &Database) -> AppResult<Vec<SessionSummary>> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(&format!(
            "{SELECT_COLUMNS} WHERE ended_at IS NULL ORDER BY started_at DESC"
        ))?;
        let rows = stmt.query_map([], row_to_summary)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    })
}

/**
 * SOURCE OF TRUTH KEYWORDS: checkpoint_transcript, crash_safe_recording
 * WHAT:  Saves the words heard so far onto a session that is still recording.
 * WHY:   The transcript used to exist only in memory until the recording ended,
 *        so a crash or a force-quit lost everything said since the start —
 *        which, on a long dictation, is the whole thing. Interior chunks are
 *        decoded while the user is still talking; writing each one's result
 *        here means an interrupted session keeps all but the last few seconds.
 *        Guarded by `ended_at IS NULL` so a late checkpoint can never overwrite
 *        a session that has already been finalised or recovered.
 * WHERE: Called by session/actor.rs after each chunk is assembled; read back by
 *        History through find_orphans / mark_orphaned.
 */
pub fn checkpoint_transcript(db: &Database, id: &SessionId, raw_text: &str) -> AppResult<()> {
    db.with_connection(|conn| {
        conn.execute(
            "UPDATE sessions SET raw_text = ?2 WHERE id = ?1 AND ended_at IS NULL",
            params![id.as_str(), raw_text],
        )?;
        Ok(())
    })
}

/// Closes out an orphan that cannot be recovered, so it stops being reported.
pub fn mark_orphaned(db: &Database, id: &SessionId, ended_at: i64) -> AppResult<()> {
    db.with_connection(|conn| {
        conn.execute(
            "UPDATE sessions SET ended_at = ?2, outcome = ?3 WHERE id = ?1 AND ended_at IS NULL",
            params![id.as_str(), ended_at, SessionOutcome::Orphaned.as_str()],
        )?;
        Ok(())
    })
}

/// Retention policy. Takes an absolute cutoff — computing it from a retention
/// setting is a business rule and belongs to the caller.
pub fn purge_older_than(db: &Database, cutoff_ms: i64) -> AppResult<u64> {
    db.with_connection(|conn| {
        let removed = conn.execute(
            "DELETE FROM sessions WHERE started_at < ?1",
            params![cutoff_ms],
        )?;
        Ok(removed as u64)
    })
}

pub fn count_sessions(db: &Database) -> AppResult<i64> {
    db.with_connection(|conn| {
        let count: i64 = conn.query_row("SELECT count(*) FROM sessions", [], |row| row.get(0))?;
        Ok(count)
    })
}

// ── Row mapping ──────────────────────────────────────────────────────────

/// Column order here is the contract `row_to_summary` reads by index.
const SELECT_COLUMNS: &str = "SELECT id, started_at, ended_at, outcome, duration_ms, language, \
     engine_id, model_id, raw_text, final_text, word_count, app_bundle_id, delivery, error_code, \
     error_message, reverted, title, edited_at FROM sessions";

fn row_to_summary(row: &Row<'_>) -> rusqlite::Result<SessionSummary> {
    let outcome: String = row.get(3)?;
    let delivery: String = row.get(12)?;

    Ok(SessionSummary {
        id: SessionId(row.get(0)?),
        started_at_ms: row.get(1)?,
        ended_at_ms: row.get(2)?,
        // An unrecognised value means the file was written by a newer build.
        // Treating it as Orphaned is the honest reading: we cannot vouch for it.
        outcome: SessionOutcome::from_stored(&outcome).unwrap_or(SessionOutcome::Orphaned),
        duration_ms: row.get(4)?,
        language: row.get(5)?,
        engine_id: row.get(6)?,
        model_id: row.get(7)?,
        raw_text: row.get(8)?,
        final_text: row.get(9)?,
        word_count: row.get(10)?,
        app_bundle_id: row.get(11)?,
        delivery: DeliveryKind::from_stored(&delivery).unwrap_or(DeliveryKind::None),
        error_code: row.get(13)?,
        error_message: row.get(14)?,
        reverted: row.get(15)?,
        title: row.get(16)?,
        edited_at_ms: row.get(17)?,
    })
}

/// Used by tests and by callers that already hold a connection.
#[allow(dead_code)]
fn row_count(conn: &Connection, table: &str) -> rusqlite::Result<i64> {
    conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row.get(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn searching(query: &str) -> HistoryFilter {
        HistoryFilter { query: Some(query.to_string()), ..HistoryFilter::default() }
    }

    fn seed(db: &Database, id: &str, started_at: i64) -> AppResult<SessionId> {
        let session_id = SessionId(id.to_string());
        create_session(
            db,
            &NewSession {
                id: session_id.clone(),
                started_at,
                engine_id: "whisper".into(),
                model_id: "turbo".into(),
                app_bundle_id: None,
            },
        )?;
        Ok(session_id)
    }

    #[test]
    fn a_new_session_is_an_orphan_until_finalized() -> AppResult<()> {
        let db = Database::open_in_memory()?;
        let id = seed(&db, "s1", 1000)?;

        // This is the crash-recovery guarantee: the row exists and is findable
        // as an orphan from the moment audio starts.
        let orphans = find_orphans(&db)?;
        assert_eq!(orphans.len(), 1);
        assert_eq!(orphans[0].id, id);

        finalize_session(
            &db,
            &id,
            &SessionResult {
                ended_at: 2000,
                outcome: SessionOutcome::Delivered,
                duration_ms: 1000,
                language: Some("en".into()),
                raw_text: Some("hello there".into()),
                final_text: Some("Hello there.".into()),
                word_count: Some(2),
                delivery: DeliveryKind::Pasted,
                error_code: None,
                error_message: None,
            },
        )?;

        assert!(find_orphans(&db)?.is_empty());
        let stored = get_session(&db, &id)?.expect("session should exist");
        assert_eq!(stored.outcome, SessionOutcome::Delivered);
        assert_eq!(stored.delivery, DeliveryKind::Pasted);
        Ok(())
    }

    #[test]
    fn search_finds_a_finalized_transcript() -> AppResult<()> {
        let db = Database::open_in_memory()?;
        let id = seed(&db, "s1", 1000)?;
        finalize_session(
            &db,
            &id,
            &SessionResult {
                ended_at: 2000,
                outcome: SessionOutcome::Delivered,
                duration_ms: 1000,
                language: Some("en".into()),
                raw_text: None,
                final_text: Some("the quick brown fox".into()),
                word_count: Some(4),
                delivery: DeliveryKind::Pasted,
                error_code: None,
                error_message: None,
            },
        )?;

        assert_eq!(list_sessions(&db, &searching("brown"), 10, 0)?.len(), 1);
        assert_eq!(list_sessions(&db, &searching("elephant"), 10, 0)?.len(), 0);
        Ok(())
    }

    #[test]
    fn deleting_a_session_takes_its_metrics_with_it() -> AppResult<()> {
        // Cancellation depends on this: one DELETE has to leave nothing behind.
        let db = Database::open_in_memory()?;
        let id = seed(&db, "s1", 1000)?;

        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO session_metrics (session_id, stage, duration_ms, recorded_at)
                 VALUES (?1, 'tail_decode', 120.0, 1500)",
                params![id.as_str()],
            )?;
            Ok(())
        })?;

        delete_session(&db, &id)?;

        db.with_connection(|conn| {
            assert_eq!(row_count(conn, "session_metrics")?, 0);
            assert_eq!(row_count(conn, "sessions")?, 0);
            Ok(())
        })
    }

    #[test]
    fn purge_removes_only_rows_older_than_the_cutoff() -> AppResult<()> {
        let db = Database::open_in_memory()?;
        seed(&db, "old", 1_000)?;
        seed(&db, "new", 9_000)?;

        assert_eq!(purge_older_than(&db, 5_000)?, 1);
        assert_eq!(count_sessions(&db)?, 1);
        Ok(())
    }

    #[test]
    fn choosing_the_original_is_a_reversible_flag_that_leaves_both_texts_alone() -> AppResult<()> {
        let db = Database::open_in_memory()?;
        let id = seed(&db, "s1", 1_000)?;
        finalize_session(
            &db,
            &id,
            &SessionResult {
                ended_at: 2_000,
                outcome: SessionOutcome::Delivered,
                duration_ms: 1_000,
                language: Some("en".into()),
                raw_text: Some("um hello there".into()),
                final_text: Some("Hello there.".into()),
                word_count: Some(2),
                delivery: DeliveryKind::Pasted,
                error_code: None,
                error_message: None,
            },
        )?;

        let before = get_session(&db, &id)?.expect("exists");
        assert!(!before.reverted);
        assert_eq!(before.effective_text(), Some("Hello there."));

        assert!(set_reverted(&db, &id, true)?);
        let reverted = get_session(&db, &id)?.expect("exists");
        assert_eq!(reverted.effective_text(), Some("um hello there"));
        assert_eq!(reverted.final_text.as_deref(), Some("Hello there."), "the delivered text is kept");

        assert!(set_reverted(&db, &id, false)?);
        assert_eq!(get_session(&db, &id)?.expect("exists").effective_text(), Some("Hello there."));
        Ok(())
    }

    #[test]
    fn editing_a_dictation_changes_what_it_stands_for_and_nothing_it_recorded() -> AppResult<()> {
        let db = Database::open_in_memory()?;
        let id = seed(&db, "s1", 1_000)?;
        finalize_session(
            &db,
            &id,
            &SessionResult {
                ended_at: 2_000,
                outcome: SessionOutcome::Delivered,
                duration_ms: 1_000,
                language: Some("en".into()),
                raw_text: Some("um hello there".into()),
                final_text: Some("Hello there.".into()),
                word_count: Some(2),
                delivery: DeliveryKind::Pasted,
                error_code: None,
                error_message: None,
            },
        )?;
        // The user had chosen the original; editing must not leave that flag
        // hiding the edit from copy and export.
        set_reverted(&db, &id, true)?;

        let saved = update_session_text(&db, &id, Some("  Greeting  "), "Hello there, friend.", 5_000)?
            .expect("the session exists");

        assert_eq!(saved.title.as_deref(), Some("Greeting"), "the title is trimmed");
        assert_eq!(saved.final_text.as_deref(), Some("Hello there, friend."));
        assert_eq!(saved.word_count, Some(3), "the word count follows the text");
        assert_eq!(saved.raw_text.as_deref(), Some("um hello there"), "what was said is a record");
        assert!(!saved.reverted, "editing means working on the saved text");
        assert_eq!(saved.effective_text(), Some("Hello there, friend."));
        assert_eq!(saved.edited_at_ms, Some(5_000));
        Ok(())
    }

    #[test]
    fn a_blank_title_is_stored_as_no_title() -> AppResult<()> {
        let db = Database::open_in_memory()?;
        let id = seed(&db, "s1", 1_000)?;
        update_session_text(&db, &id, Some("Named"), "text", 2_000)?;
        let cleared = update_session_text(&db, &id, Some("   "), "text", 3_000)?.expect("exists");
        assert_eq!(cleared.title, None);
        Ok(())
    }

    #[test]
    fn a_renamed_dictation_is_found_by_its_new_name_and_not_its_old_one() -> AppResult<()> {
        let db = Database::open_in_memory()?;
        let id = seed(&db, "s1", 1_000)?;
        update_session_text(&db, &id, Some("Standup notes"), "we shipped the thing", 2_000)?;

        assert_eq!(list_sessions(&db, &searching("standup"), 10, 0)?.len(), 1, "found by title");
        assert_eq!(list_sessions(&db, &searching("shipped"), 10, 0)?.len(), 1, "found by text");

        update_session_text(&db, &id, Some("Retro"), "we shipped the thing", 3_000)?;
        assert!(list_sessions(&db, &searching("standup"), 10, 0)?.is_empty(), "the old title left the index");
        assert_eq!(list_sessions(&db, &searching("retro"), 10, 0)?.len(), 1);
        Ok(())
    }

    #[test]
    fn editing_a_session_that_does_not_exist_reports_it() -> AppResult<()> {
        let db = Database::open_in_memory()?;
        assert!(update_session_text(&db, &SessionId("nope".into()), None, "x", 1)?.is_none());
        Ok(())
    }

    #[test]
    fn reverting_a_session_that_does_not_exist_reports_it() -> AppResult<()> {
        let db = Database::open_in_memory()?;
        assert!(!set_reverted(&db, &SessionId("nope".into()), true)?);
        Ok(())
    }

    #[test]
    fn the_latest_transcript_is_the_newest_one_that_heard_something() -> AppResult<()> {
        let db = Database::open_in_memory()?;
        assert!(latest_with_text(&db)?.is_none(), "an empty history has nothing to copy");

        let older = seed(&db, "older", 1_000)?;
        let newer = seed(&db, "newer-but-silent", 2_000)?;
        let result = |text: Option<&str>| SessionResult {
            ended_at: 3_000,
            outcome: SessionOutcome::Delivered,
            duration_ms: 1_000,
            language: None,
            raw_text: text.map(Into::into),
            final_text: text.map(Into::into),
            word_count: Some(1),
            delivery: DeliveryKind::Pasted,
            error_code: None,
            error_message: None,
        };
        finalize_session(&db, &older, &result(Some("hello")))?;
        finalize_session(&db, &newer, &result(Some("   ")))?;

        let found = latest_with_text(&db)?.expect("the older session has words");
        assert_eq!(found.id, older, "a blank newer session is skipped");
        Ok(())
    }

    #[test]
    fn an_interrupted_session_keeps_the_words_checkpointed_before_it_died() -> AppResult<()> {
        let db = Database::open_in_memory()?;
        let id = seed(&db, "crashed", 1_000)?;

        checkpoint_transcript(&db, &id, "first part")?;
        checkpoint_transcript(&db, &id, "first part and the second part")?;

        // What launch-time recovery does to a row with no ended_at.
        let orphan = find_orphans(&db)?.into_iter().next().expect("still open");
        assert_eq!(orphan.raw_text.as_deref(), Some("first part and the second part"));
        mark_orphaned(&db, &id, 5_000)?;

        let recovered = get_session(&db, &id)?.expect("exists");
        assert_eq!(recovered.outcome, SessionOutcome::Orphaned);
        assert_eq!(recovered.effective_text(), Some("first part and the second part"));
        assert_eq!(latest_with_text(&db)?.map(|s| s.id), Some(id));
        Ok(())
    }

    #[test]
    fn a_late_checkpoint_cannot_overwrite_a_finished_session() -> AppResult<()> {
        let db = Database::open_in_memory()?;
        let id = seed(&db, "done", 1_000)?;
        finalize_session(
            &db,
            &id,
            &SessionResult {
                ended_at: 2_000,
                outcome: SessionOutcome::Delivered,
                duration_ms: 1_000,
                language: None,
                raw_text: Some("the real transcript".into()),
                final_text: Some("The real transcript.".into()),
                word_count: Some(3),
                delivery: DeliveryKind::Pasted,
                error_code: None,
                error_message: None,
            },
        )?;

        checkpoint_transcript(&db, &id, "stale partial")?;

        assert_eq!(
            get_session(&db, &id)?.expect("exists").raw_text.as_deref(),
            Some("the real transcript")
        );
        Ok(())
    }

    #[test]
    fn the_list_narrows_by_time_and_status_and_still_pages() -> AppResult<()> {
        let db = Database::open_in_memory()?;
        let old = seed(&db, "old", 1_000)?;
        let edited = seed(&db, "edited", 5_000)?;
        let failed = seed(&db, "failed", 6_000)?;
        update_session_text(&db, &edited, None, "hand written", 5_500)?;
        finalize_session(
            &db,
            &failed,
            &SessionResult {
                ended_at: 6_100,
                outcome: SessionOutcome::Failed,
                duration_ms: 100,
                language: None,
                raw_text: None,
                final_text: None,
                word_count: None,
                delivery: DeliveryKind::None,
                error_code: Some("X".into()),
                error_message: Some("no".into()),
            },
        )?;
        let _ = old;

        let ids = |filter: &HistoryFilter| -> AppResult<Vec<String>> {
            Ok(list_sessions(&db, filter, 10, 0)?.into_iter().map(|s| s.id.0).collect())
        };

        assert_eq!(ids(&HistoryFilter::default())?, ["failed", "edited", "old"]);
        assert_eq!(ids(&HistoryFilter { since_ms: Some(5_000), ..Default::default() })?, ["failed", "edited"]);
        assert_eq!(ids(&HistoryFilter { status: HistoryStatus::Failed, ..Default::default() })?, ["failed"]);
        assert_eq!(ids(&HistoryFilter { status: HistoryStatus::Edited, ..Default::default() })?, ["edited"]);

        let second_page = list_sessions(&db, &HistoryFilter::default(), 2, 2)?;
        assert_eq!(second_page.len(), 1, "an offset pages through a filtered or unfiltered list alike");
        Ok(())
    }
}
