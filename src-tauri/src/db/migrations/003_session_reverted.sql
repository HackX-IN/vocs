-- SOURCE OF TRUTH KEYWORDS: reverted, sessions, use_original, schema_v3
-- WHAT:  Adds the flag recording that the user chose a transcript's original
--        wording over what the enhancement rules produced.
-- WHY:   History keeps `raw_text` beside `final_text` so a bad result is
--        attributable, but nothing said which one the USER wanted. That choice
--        is the only direct signal of whether the corrections are welcome: it is
--        what "use original" writes, what copy and export honour, and what any
--        later learning from accepted and rejected corrections will read.
--        Stored as a flag rather than by overwriting final_text, so choosing
--        the original is reversible and the delivered text is never lost.
--        NOT NULL DEFAULT 0 so every existing row reads as "kept the fix".
-- WHERE: Written by services/sessions.rs::set_reverted; read by History.

ALTER TABLE sessions ADD COLUMN reverted INTEGER NOT NULL DEFAULT 0;
