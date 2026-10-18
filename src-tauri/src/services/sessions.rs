/*!
 * SOURCE OF TRUTH KEYWORDS: create_session, finalize_session, get_session,
 *   list_sessions, search_sessions, delete_session, delete_all_sessions,
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
use crate::types::{DeliveryKind, SessionId, SessionOutcome, SessionSummary};

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

/// Newest first. Cancelled sessions do not exist, so no filter is needed for
/// them; orphans and failures DO appear, which is the point of history.
pub fn list_sessions(db: &Database, limit: i64, offset: i64) -> AppResult<Vec<SessionSummary>> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(&format!(
            "{SELECT_COLUMNS} ORDER BY started_at DESC LIMIT ?1 OFFSET ?2"
        ))?;
        let rows = stmt.query_map(params![limit, offset], row_to_summary)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    })
}

/**
 * SOURCE OF TRUTH KEYWORDS: search_sessions
 * WHAT:  Full-text search over final transcripts, newest first.
 * WHY:   Goes through the FTS index rather than a LIKE scan, which stops being
 *        usable somewhere in the low thousands of rows — and this table is
 *        expected to reach tens of thousands.
 * WHERE: The History view's search box.
 */
pub fn search_sessions(db: &Database, query: &str, limit: i64) -> AppResult<Vec<SessionSummary>> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(&format!(
            "{SELECT_COLUMNS}
             WHERE rowid IN (SELECT rowid FROM sessions_fts WHERE sessions_fts MATCH ?1)
             ORDER BY started_at DESC
             LIMIT ?2"
        ))?;
        let rows = stmt.query_map(params![query, limit], row_to_summary)?;
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
     error_message, reverted FROM sessions";

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

        assert_eq!(search_sessions(&db, "brown", 10)?.len(), 1);
        assert_eq!(search_sessions(&db, "elephant", 10)?.len(), 0);
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
}
