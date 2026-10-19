-- SOURCE OF TRUTH KEYWORDS: learned_corrections, grammar_edits, learned_settled,
--   schema_v7, personal_mistake_dictionary
-- WHAT:  The table of corrections the user has taught Vocs by keeping them, and
--        the two columns a session needs so that learning can happen after the
--        fact.
-- WHY:   `grammar_edits` is the JSON list of fixes a dictation received (from the
--        grammar model or the learned list), written at delivery. They cannot be
--        judged at delivery — whether a fix was wanted is only known later, when
--        the user has or has not undone it — so the session carries them and
--        `learned_settled` says whether they have been counted yet:
--        0 not yet, 1 counted as kept, 2 counted as undone.
--        `manual` stops the arithmetic from overriding a switch the user flipped.
--        (pattern, replacement) is unique so the same fix seen twice is one row
--        with a tally, not two rows.
-- WHERE: Written by services/learned.rs and services/sessions.rs; settled by
--        session/learning.rs.

CREATE TABLE learned_corrections (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern     TEXT    NOT NULL,
    replacement TEXT    NOT NULL,
    accepted    INTEGER NOT NULL DEFAULT 0,
    rejected    INTEGER NOT NULL DEFAULT 0,
    auto_apply  INTEGER NOT NULL DEFAULT 0,
    manual      INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    UNIQUE (pattern, replacement)
);

ALTER TABLE sessions ADD COLUMN grammar_edits TEXT;
ALTER TABLE sessions ADD COLUMN learned_settled INTEGER NOT NULL DEFAULT 0;

-- The sweep reads only sessions that have edits and have not been counted.
CREATE INDEX idx_sessions_unsettled ON sessions (learned_settled, ended_at)
    WHERE grammar_edits IS NOT NULL;
