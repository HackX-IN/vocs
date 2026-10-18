-- SOURCE OF TRUTH KEYWORDS: title, edited_at, sessions, sessions_fts, schema_v5,
--   dictation_editor
-- WHAT:  Lets a dictation be named and edited: adds an optional user title and the
--        time of the last manual edit, and makes the title searchable.
-- WHY:   History became a notebook — opening a dictation shows an editor, and
--        the words in it are the user's to change. `title` is NULL until the
--        user names one, so every existing row keeps reading as "first line of
--        the text" with no backfill. `edited_at` records that the words are no
--        longer exactly what the rules produced, which is what lets the editor
--        say so honestly instead of presenting a hand-edited transcript as
--        machine output.
--
--        The search index has to learn about the title in the same migration or
--        a renamed dictation would be unfindable by the very name the user gave
--        it. An external-content FTS table cannot gain a column in place, so it
--        is dropped and rebuilt from `sessions`, triggers included — the
--        triggers are mandatory, not an optimisation (see 001).
-- WHERE: Written by services/sessions.rs::update_session_text; read by History.

ALTER TABLE sessions ADD COLUMN title TEXT;
ALTER TABLE sessions ADD COLUMN edited_at INTEGER;

DROP TRIGGER sessions_fts_insert;
DROP TRIGGER sessions_fts_delete;
DROP TRIGGER sessions_fts_update;
DROP TABLE sessions_fts;

CREATE VIRTUAL TABLE sessions_fts USING fts5 (
    title,
    final_text,
    content = 'sessions',
    content_rowid = 'rowid'
);

INSERT INTO sessions_fts (sessions_fts) VALUES ('rebuild');

CREATE TRIGGER sessions_fts_insert AFTER INSERT ON sessions BEGIN
    INSERT INTO sessions_fts (rowid, title, final_text) VALUES (new.rowid, new.title, new.final_text);
END;

CREATE TRIGGER sessions_fts_delete AFTER DELETE ON sessions BEGIN
    INSERT INTO sessions_fts (sessions_fts, rowid, title, final_text)
    VALUES ('delete', old.rowid, old.title, old.final_text);
END;

CREATE TRIGGER sessions_fts_update AFTER UPDATE ON sessions BEGIN
    INSERT INTO sessions_fts (sessions_fts, rowid, title, final_text)
    VALUES ('delete', old.rowid, old.title, old.final_text);
    INSERT INTO sessions_fts (rowid, title, final_text) VALUES (new.rowid, new.title, new.final_text);
END;
