-- SOURCE OF TRUTH KEYWORDS: excluded, app_profiles, excluded_apps, schema_v4
-- WHAT:  Adds the flag that turns Vocs off while a particular app is in front.
-- WHY:   A password manager or a terminal is somewhere dictating is either
--        unwanted or unsafe, and "override a dozen settings so nothing happens"
--        is not a way to say that. Stored on the profile rather than in a new
--        table because the key is the same (the app's bundle id) and a profile
--        already carries the app's name, its enabled switch and its place in
--        Settings; a second list would mean two places to look for what Vocs
--        does in one app. NOT NULL DEFAULT 0 so every existing profile keeps
--        behaving exactly as before.
-- WHERE: Read by session/settings_view.rs when a session starts; edited in
--        Settings -> Per-app profiles.

ALTER TABLE app_profiles ADD COLUMN excluded INTEGER NOT NULL DEFAULT 0;
