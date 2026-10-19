/*!
 * SOURCE OF TRUTH KEYWORDS: learning, settle_due, on_reverted, judge,
 *   record_session_edits, should_auto_apply, SETTLE_AFTER_MS,
 *   personal_mistake_dictionary
 * WHAT:  Learns from what the user does with the fixes Vocs made: counts each
 *        one as kept or undone, and promotes a fix that has been kept three times
 *        and never undone to one that applies itself.
 * WHY:   Learning from the model's say-so would teach Vocs to repeat the
 *        model's mistakes. The signal that a fix was WANTED is behavioural: the
 *        person saw the corrected text and left it. So a session carries its
 *        fixes (written at delivery) and is judged later, once the person has had
 *        time to notice and undo — `SETTLE_AFTER_MS` — by looking at the text as
 *        it now stands: still holding the fix is kept, "Use original" or an edit
 *        that removed it is undone.
 *
 *        Choosing the original is counted at once rather than at the next sweep,
 *        because that click is the clearest "no" the user can give and an
 *        auto-applied fix must stop applying the moment it is given. A later
 *        change of mind (restoring the fix) reverses it.
 *
 *        The promotion rule lives here and not in the service so it can change
 *        without a migration; a fix the user set by hand is never moved by it.
 * WHERE: record_session_edits is called by session/delivery.rs; on_reverted by
 *        ipc/commands/history.rs; run_learning_housekeeping is spawned by
 *        bootstrap.
 */

use std::time::Duration;

use crate::db::Database;
use crate::error::AppResult;
use crate::pipeline::grammar::learned::contains_phrase;
use crate::services;
use crate::telemetry::now_ms;
use crate::types::{CorrectionEdit, LearnedCorrection, SessionId, AUTO_APPLY_AFTER};

/// Long enough for the person to have read the result and undone it if it was
/// wrong; short enough that a day of dictation settles the same day.
pub const SETTLE_AFTER_MS: i64 = 30 * 60 * 1000;
const SWEEP_EVERY: Duration = Duration::from_secs(10 * 60);
const SWEEP_BATCH: i64 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Kept,
    Undone,
}

/// Whether the person kept this fix, judged from the session as it stands.
pub fn judge(edit: &CorrectionEdit, final_text: &str, reverted: bool) -> Verdict {
    if !reverted && contains_phrase(final_text, &edit.replacement) {
        Verdict::Kept
    } else {
        Verdict::Undone
    }
}

/// Kept enough, never undone.
pub fn should_auto_apply(correction: &LearnedCorrection) -> bool {
    correction.accepted >= AUTO_APPLY_AFTER && correction.rejected == 0
}

fn parse(json: &str) -> Vec<CorrectionEdit> {
    serde_json::from_str(json).unwrap_or_default()
}

/// Adds to one fix's tallies and re-evaluates whether it applies itself.
fn count(db: &Database, edit: &CorrectionEdit, kept: i64, undone: i64) -> AppResult<()> {
    let row = services::learned::upsert_counts(db, &edit.pattern, &edit.replacement, kept, undone, now_ms())?;
    if !row.manual {
        let auto = should_auto_apply(&row);
        if auto != row.auto_apply {
            services::learned::set_auto(db, row.id, auto, false)?;
        }
    }
    Ok(())
}

/**
 * WHAT:  Stores the fixes a delivered dictation received, and makes sure each
 *        proposed one has a row so the user can see what is being learned.
 * WHY:   Fixes that were applied from the learned list are stored too (flagged)
 *        but get no new row — they already have one — so that undoing the
 *        session can count against them.
 */
pub fn record_session_edits(db: &Database, session_id: &SessionId, edits: &[CorrectionEdit]) {
    if edits.is_empty() {
        return;
    }
    let result = serde_json::to_string(edits)
        .map_err(crate::error::AppError::internal)
        .and_then(|json| services::sessions::set_grammar_edits(db, session_id, Some(&json)))
        .and_then(|()| {
            for edit in edits.iter().filter(|e| !e.auto) {
                services::learned::upsert_counts(db, &edit.pattern, &edit.replacement, 0, 0, now_ms())?;
            }
            Ok(())
        });
    if let Err(err) = result {
        tracing::warn!(error = %err, "could not record a dictation's grammar fixes");
    }
}

/// Counts every session old enough to judge. Returns how many it settled.
pub fn settle_due(db: &Database, now: i64) -> usize {
    let due = match services::sessions::unsettled_edits(db, now - SETTLE_AFTER_MS, SWEEP_BATCH) {
        Ok(due) => due,
        Err(err) => {
            tracing::warn!(error = %err, "could not look for dictations to learn from");
            return 0;
        }
    };

    for state in &due {
        for edit in parse(&state.edits_json) {
            let (kept, undone) = match judge(&edit, &state.final_text, state.reverted) {
                Verdict::Kept => (1, 0),
                Verdict::Undone => (0, 1),
            };
            if let Err(err) = count(db, &edit, kept, undone) {
                tracing::warn!(error = %err, "could not count a grammar fix");
            }
        }
        if let Err(err) = services::sessions::set_learned_settled(db, &state.id, 1) {
            tracing::warn!(error = %err, "could not mark a dictation as learned from");
        }
    }
    due.len()
}

/**
 * WHAT:  Applies the user's "use original" choice (or taking it back) to the
 *        tallies immediately.
 * WHY:   See the module WHY. The three starting states are exactly: not counted
 *        yet, counted as kept, counted as undone.
 * WHERE: ipc/commands/history.rs, after set_reverted.
 */
pub fn on_reverted(db: &Database, session_id: &SessionId, reverted: bool) {
    let Ok(Some(state)) = services::sessions::get_edit_state(db, session_id) else {
        return;
    };
    let edits = parse(&state.edits_json);

    let (kept, undone, next) = match (reverted, state.settled) {
        // Undone before it was ever counted as kept.
        (true, 0) => (0, 1, 2),
        // Counted as kept earlier; the user has changed their mind.
        (true, 1) => (-1, 1, 2),
        // Taken back: forget the "no" and let the next sweep judge afresh.
        (false, 2) => (0, -1, 0),
        _ => return,
    };

    for edit in &edits {
        if let Err(err) = count(db, edit, kept, undone) {
            tracing::warn!(error = %err, "could not count a reverted grammar fix");
        }
    }
    let _ = services::sessions::set_learned_settled(db, session_id, next);
}

/// Runs for the life of the app: judge what is due at launch, then every ten minutes.
pub async fn run_learning_housekeeping(db: Database) {
    let mut ticker = tokio::time::interval(SWEEP_EVERY);
    loop {
        ticker.tick().await;
        let db = db.clone();
        let settled = tokio::task::spawn_blocking(move || settle_due(&db, now_ms())).await.unwrap_or(0);
        if settled > 0 {
            tracing::debug!(settled, "learned from settled dictations");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::sessions::{create_session, finalize_session, NewSession, SessionResult};
    use crate::types::{DeliveryKind, SessionOutcome};

    fn edit(pattern: &str, replacement: &str) -> CorrectionEdit {
        CorrectionEdit { pattern: pattern.into(), replacement: replacement.into(), auto: false }
    }

    /// A finished session `age_ms` old, carrying `edits`.
    fn session(db: &Database, id: &str, final_text: &str, edits: &[CorrectionEdit], age_ms: i64) -> SessionId {
        let sid = SessionId(id.into());
        let started = now_ms() - age_ms;
        create_session(
            db,
            &NewSession {
                id: sid.clone(),
                started_at: started,
                engine_id: "e".into(),
                model_id: "m".into(),
                app_bundle_id: None,
            },
        )
        .expect("created");
        finalize_session(
            db,
            &sid,
            &SessionResult {
                ended_at: started + 1_000,
                outcome: SessionOutcome::Delivered,
                duration_ms: 1_000,
                language: Some("en".into()),
                raw_text: Some("he go home".into()),
                final_text: Some(final_text.into()),
                word_count: Some(3),
                delivery: DeliveryKind::Pasted,
                error_code: None,
                error_message: None,
            },
        )
        .expect("finalized");
        record_session_edits(db, &sid, edits);
        sid
    }

    fn row(db: &Database, pattern: &str) -> LearnedCorrection {
        services::learned::list(db).expect("list").into_iter().find(|c| c.pattern == pattern).expect("row")
    }

    #[test]
    fn a_fix_the_person_kept_is_counted_and_three_keeps_make_it_automatic() {
        let db = Database::open_in_memory().expect("db");
        let fix = edit("he go", "he goes");
        for n in 0..3 {
            session(&db, &format!("s{n}"), "Then he goes home.", std::slice::from_ref(&fix), SETTLE_AFTER_MS + 60_000);
        }

        assert_eq!(settle_due(&db, now_ms()), 3);

        let learned = row(&db, "he go");
        assert_eq!((learned.accepted, learned.rejected), (3, 0));
        assert!(learned.auto_apply, "three keeps and no undo makes the fix automatic");
    }

    #[test]
    fn a_recent_dictation_is_not_judged_yet() {
        let db = Database::open_in_memory().expect("db");
        session(&db, "fresh", "he goes home", &[edit("he go", "he goes")], 1_000);
        assert_eq!(settle_due(&db, now_ms()), 0);
        assert_eq!(row(&db, "he go").accepted, 0, "it was recorded but not yet counted");
    }

    #[test]
    fn a_session_is_counted_once() {
        let db = Database::open_in_memory().expect("db");
        session(&db, "once", "he goes home", &[edit("he go", "he goes")], SETTLE_AFTER_MS + 60_000);
        settle_due(&db, now_ms());
        settle_due(&db, now_ms());
        assert_eq!(row(&db, "he go").accepted, 1);
    }

    #[test]
    fn editing_the_fix_away_counts_as_undone() {
        let db = Database::open_in_memory().expect("db");
        session(&db, "edited", "Then he go home.", &[edit("he go", "he goes")], SETTLE_AFTER_MS + 60_000);
        settle_due(&db, now_ms());
        let learned = row(&db, "he go");
        assert_eq!((learned.accepted, learned.rejected), (0, 1));
        assert!(!learned.auto_apply);
    }

    #[test]
    fn using_the_original_counts_at_once_and_switches_an_automatic_fix_off() {
        let db = Database::open_in_memory().expect("db");
        let fix = edit("he go", "he goes");
        for n in 0..3 {
            session(&db, &format!("k{n}"), "he goes home", std::slice::from_ref(&fix), SETTLE_AFTER_MS + 60_000);
        }
        settle_due(&db, now_ms());
        assert!(row(&db, "he go").auto_apply);

        // The fourth dictation applied it automatically; the person undoes it.
        let auto = CorrectionEdit { auto: true, ..fix.clone() };
        let sid = session(&db, "applied", "he goes home", &[auto], 1_000);
        services::sessions::set_reverted(&db, &sid, true).expect("reverted");
        on_reverted(&db, &sid, true);

        let learned = row(&db, "he go");
        assert_eq!(learned.rejected, 1);
        assert!(!learned.auto_apply, "one \"no\" stops it applying itself");
    }

    #[test]
    fn taking_back_a_revert_forgets_the_no() {
        let db = Database::open_in_memory().expect("db");
        let sid = session(&db, "flip", "he goes home", &[edit("he go", "he goes")], 1_000);
        on_reverted(&db, &sid, true);
        assert_eq!(row(&db, "he go").rejected, 1);
        on_reverted(&db, &sid, false);
        assert_eq!(row(&db, "he go").rejected, 0);
    }

    #[test]
    fn a_choice_the_user_made_by_hand_is_not_moved_by_the_tallies() {
        let db = Database::open_in_memory().expect("db");
        let fix = edit("he go", "he goes");
        session(&db, "manual", "he goes home", std::slice::from_ref(&fix), SETTLE_AFTER_MS + 60_000);
        let id = row(&db, "he go").id;
        services::learned::set_auto(&db, id, false, true).expect("set");

        for n in 0..4 {
            session(&db, &format!("m{n}"), "he goes home", std::slice::from_ref(&fix), SETTLE_AFTER_MS + 60_000);
        }
        settle_due(&db, now_ms());

        let learned = row(&db, "he go");
        assert!(learned.accepted >= 3);
        assert!(!learned.auto_apply, "the user said no automatic; arithmetic cannot say yes");
    }

    #[test]
    fn the_promotion_rule_is_three_keeps_and_no_undo() {
        let mut c = LearnedCorrection {
            id: crate::types::LearnedId(1),
            pattern: "a b".into(),
            replacement: "a c".into(),
            accepted: 2,
            rejected: 0,
            auto_apply: false,
            manual: false,
            updated_at: 0,
        };
        assert!(!should_auto_apply(&c));
        c.accepted = 3;
        assert!(should_auto_apply(&c));
        c.rejected = 1;
        assert!(!should_auto_apply(&c));
    }
}
