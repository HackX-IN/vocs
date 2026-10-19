/*!
 * SOURCE OF TRUTH KEYWORDS: learned_corrections_service, upsert_counts, auto_applied,
 *   list_learned, set_auto
 * WHAT:  Reads and writes the learned_corrections table.
 * WHY:   Counters and a flag, nothing else. Whether a tally is enough to make a
 *        fix automatic is a RULE and lives in session/learning.rs; this layer
 *        only stores what it is told, which is what keeps the policy changeable
 *        without a migration.
 * WHERE: Called by session/learning.rs, session/grammar_step.rs and
 *        ipc/commands/learned.rs.
 */

use rusqlite::{params, Row};

use crate::db::Database;
use crate::error::AppResult;
use crate::types::{LearnedCorrection, LearnedId};

const SELECT_COLUMNS: &str =
    "SELECT id, pattern, replacement, accepted, rejected, auto_apply, manual, updated_at FROM learned_corrections";

fn row_to_correction(row: &Row<'_>) -> rusqlite::Result<LearnedCorrection> {
    Ok(LearnedCorrection {
        id: LearnedId(row.get(0)?),
        pattern: row.get(1)?,
        replacement: row.get(2)?,
        accepted: row.get::<_, i64>(3)?.max(0) as u32,
        rejected: row.get::<_, i64>(4)?.max(0) as u32,
        auto_apply: row.get(5)?,
        manual: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

/// Adds to a fix's tallies, creating it at zero first, and returns the result.
/// Tallies never go below zero: a correction undone more often than it was
/// counted is a bookkeeping slip, not a reason to carry a negative.
pub fn upsert_counts(
    db: &Database,
    pattern: &str,
    replacement: &str,
    accepted_delta: i64,
    rejected_delta: i64,
    now: i64,
) -> AppResult<LearnedCorrection> {
    db.with_connection(|conn| {
        conn.execute(
            "INSERT OR IGNORE INTO learned_corrections (pattern, replacement, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?3)",
            params![pattern, replacement, now],
        )?;
        conn.execute(
            "UPDATE learned_corrections SET \
                accepted = max(0, accepted + ?3), rejected = max(0, rejected + ?4), updated_at = ?5 \
             WHERE pattern = ?1 AND replacement = ?2",
            params![pattern, replacement, accepted_delta, rejected_delta, now],
        )?;
        let row = conn.query_row(
            &format!("{SELECT_COLUMNS} WHERE pattern = ?1 AND replacement = ?2"),
            params![pattern, replacement],
            row_to_correction,
        )?;
        Ok(row)
    })
}

/// `manual` marks a choice the user made, which the tallies must not override.
pub fn set_auto(db: &Database, id: LearnedId, auto_apply: bool, manual: bool) -> AppResult<bool> {
    db.with_connection(|conn| {
        let changed = conn.execute(
            "UPDATE learned_corrections SET auto_apply = ?2, manual = ?3 WHERE id = ?1",
            params![id.0, auto_apply, manual],
        )?;
        Ok(changed > 0)
    })
}

/// Most-kept first, so the list leads with what Vocs is surest of.
pub fn list(db: &Database) -> AppResult<Vec<LearnedCorrection>> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(&format!(
            "{SELECT_COLUMNS} ORDER BY auto_apply DESC, accepted DESC, updated_at DESC"
        ))?;
        let rows = stmt.query_map([], row_to_correction)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    })
}

/// The fixes to apply without the model.
pub fn auto_applied(db: &Database) -> AppResult<Vec<LearnedCorrection>> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(&format!(
            "{SELECT_COLUMNS} WHERE auto_apply = 1 ORDER BY length(pattern) DESC"
        ))?;
        let rows = stmt.query_map([], row_to_correction)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    })
}

pub fn delete(db: &Database, id: LearnedId) -> AppResult<()> {
    db.with_connection(|conn| {
        conn.execute("DELETE FROM learned_corrections WHERE id = ?1", params![id.0])?;
        Ok(())
    })
}

pub fn clear(db: &Database) -> AppResult<()> {
    db.with_connection(|conn| {
        conn.execute("DELETE FROM learned_corrections", [])?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_fix_seen_twice_is_one_row_with_a_tally() -> AppResult<()> {
        let db = Database::open_in_memory()?;
        upsert_counts(&db, "he go", "he goes", 1, 0, 1)?;
        let row = upsert_counts(&db, "he go", "he goes", 1, 0, 2)?;
        assert_eq!((row.accepted, row.rejected), (2, 0));
        assert_eq!(list(&db)?.len(), 1);
        Ok(())
    }

    #[test]
    fn tallies_never_go_negative() -> AppResult<()> {
        let db = Database::open_in_memory()?;
        let row = upsert_counts(&db, "he go", "he goes", -5, -1, 1)?;
        assert_eq!((row.accepted, row.rejected), (0, 0));
        Ok(())
    }

    #[test]
    fn only_fixes_marked_automatic_are_returned_for_application() -> AppResult<()> {
        let db = Database::open_in_memory()?;
        let one = upsert_counts(&db, "he go", "he goes", 3, 0, 1)?;
        upsert_counts(&db, "she dont", "she doesn't", 1, 0, 1)?;
        assert!(auto_applied(&db)?.is_empty());

        set_auto(&db, one.id, true, false)?;
        let applied = auto_applied(&db)?;
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].pattern, "he go");
        Ok(())
    }

    #[test]
    fn a_manual_choice_is_recorded_and_delete_and_clear_remove_rows() -> AppResult<()> {
        let db = Database::open_in_memory()?;
        let a = upsert_counts(&db, "a b", "a c", 0, 0, 1)?;
        let b = upsert_counts(&db, "d e", "d f", 0, 0, 1)?;
        assert!(set_auto(&db, a.id, true, true)?);
        assert!(list(&db)?.iter().any(|c| c.id == a.id && c.manual && c.auto_apply));
        delete(&db, b.id)?;
        assert_eq!(list(&db)?.len(), 1);
        clear(&db)?;
        assert!(list(&db)?.is_empty());
        assert!(!set_auto(&db, a.id, true, true)?, "a missing row reports false");
        Ok(())
    }
}
