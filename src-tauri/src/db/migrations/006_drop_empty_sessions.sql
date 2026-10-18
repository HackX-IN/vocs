-- SOURCE OF TRUTH KEYWORDS: empty_sessions, nothing_said, schema_v6, history_noise
-- WHAT:  Removes dictations that recorded no words and failed at nothing.
-- WHY:   A recording that heard nothing (a tap of the hotkey, a pause that ended
--        the session, a crash before the first chunk) used to be stored as a
--        delivered dictation with no text. History listed them as blank rows
--        and Stats counted them as dictations. The app no longer writes them
--        (session/delivery.rs deletes the row), so the rows already on disk are
--        cleaned up once, here. Failures are kept — they carry an error the user
--        and we may need — and anything with any recovered words is kept.
-- WHERE: Applied once by db/migrations.rs.

DELETE FROM sessions
 WHERE outcome IN ('delivered', 'orphaned')
   AND coalesce(trim(final_text), '') = ''
   AND coalesce(trim(raw_text), '') = '';
