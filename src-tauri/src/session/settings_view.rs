/*!
 * SOURCE OF TRUTH KEYWORDS: SessionSettings, load, read_bool, read_number,
 *   read_choice, effective_setting
 * WHAT:  A typed snapshot of every setting one recording session needs.
 * WHY:   Read ONCE at session start rather than queried per chunk. Two reasons,
 *        and the second is the important one:
 *          - The pipeline touches these on the latency-critical path, and a
 *            SQLite round trip per chunk would be measurable.
 *          - Settings changing mid-recording would mean a session behaved one
 *            way at the start and another at the end. A snapshot makes each
 *            session internally consistent, which is also what makes a bug
 *            report reproducible.
 *
 *        Every read falls back to the registry's declared default, so a setting
 *        that has never been written — which is every setting on a fresh
 *        install — resolves without a special case.
 * WHERE: Built by the session actor; refreshed when SettingsChanged fires.
 */

use crate::db::Database;
use crate::registry::{self, keys};
use crate::services;
use crate::types::{CaptureMode, LanguageCode, SettingValue};

/// Guard against a stored value that would make the app unusable.
const MIN_COUNTDOWN_MS: u64 = 1_000;
const MAX_COUNTDOWN_MS: u64 = 30_000;
/// A preview shorter than this cannot be read, longer than this is a delay.
const MIN_PREVIEW_MS: u64 = 500;
const MAX_PREVIEW_MS: u64 = 5_000;

#[derive(Debug, Clone)]
pub struct SessionSettings {
    pub model_id: String,
    /// None means auto-detect. An explicit choice always beats detection.
    pub language: Option<LanguageCode>,
    pub input_device: Option<String>,
    pub capture_mode: CaptureMode,
    pub cancel_countdown_ms: u64,
    /// Escape destroys the recording at once rather than arming a countdown.
    pub discard_on_escape: bool,
    pub finalize_timeout_ms: u64,
    pub auto_paste: bool,
    pub restore_clipboard: bool,
    /// Milliseconds. Per-app, which is why they live here and travel on the
    /// InjectionRequest rather than being held by the injector.
    pub paste_delay_ms: u64,
    pub clipboard_restore_delay_ms: u64,
    pub strip_fillers: bool,
    pub filler_strength: crate::types::FillerStrength,
    pub spoken_commands: bool,
    pub apply_corrections: bool,
    pub fix_grammar: bool,
    /// Run the on-device grammar model after the rules. Needs the model installed.
    pub ai_grammar: bool,
    /// Remember fixes the user keeps, and apply the ones kept three times.
    pub learn_corrections: bool,
    pub tone: crate::types::Tone,
    /// Show the finished text in the caption HUD for `preview_ms` before pasting.
    pub preview_before_paste: bool,
    pub preview_ms: u64,
    /// Transcribe interim snapshots while recording and show them as live captions.
    pub live_captions: bool,
    pub normalise_punctuation: bool,
    pub capitalise_sentences: bool,
    pub audio_feedback: bool,
    /// Mute the built-in speakers for the length of a recording.
    pub quiet_speakers: bool,
    /// The user's own recognition bias, ahead of the dictionary terms. None
    /// when blank, so the actor never builds a prompt out of whitespace.
    pub context_prompt: Option<String>,
    /// How hard the engine works on each decode. See types/accuracy.rs.
    pub accuracy: crate::types::AccuracyMode,
    /// The display name of the frontmost app when its profile turns Vocs off.
    /// None everywhere else, including when no app could be identified.
    pub excluded_app: Option<String>,
}

impl SessionSettings {
    /// Every setting resolved against the registry defaults. Takes the
    /// database rather than AppState because bootstrap needs this BEFORE
    /// AppState exists — the actor it configures is what AppState points at.
    pub fn load(db: &Database) -> Self {
        Self::from_stored(&services::settings::get_all_settings(db).unwrap_or_default())
    }

    /**
     * SOURCE OF TRUTH KEYWORDS: load_for_app, per_app_profile
     * WHAT:  Global settings with the frontmost app's profile layered on top.
     * WHY:   Layering, not replacing. A profile is sparse — it names only what
     *        it changes — so everything it does not mention keeps following the
     *        global setting. That is what stops an app profile created today
     *        from silently freezing every other preference at today's value.
     *
     *        Applied at session START and never mid-session, so a session's
     *        behaviour is decided once by the app that had focus when the
     *        hotkey fired. Switching apps while talking must not change the
     *        rules halfway through.
     * WHERE: Called by the session actor when a recording begins.
     */
    pub fn load_for_app(db: &Database, bundle_id: Option<&str>) -> Self {
        let mut stored = services::settings::get_all_settings(db).unwrap_or_default();
        let mut excluded_app = None;

        if let Some(bundle_id) = bundle_id {
            match services::profiles::get_profile(db, bundle_id) {
                Ok(Some(profile)) => {
                    tracing::debug!(bundle_id, overrides = profile.overrides.len(), "applying app profile");
                    if profile.excluded {
                        excluded_app = Some(profile.display_name.clone());
                    }
                    stored.extend(profile.overrides);
                }
                Ok(None) => {}
                Err(err) => tracing::warn!(error = %err, bundle_id, "could not read app profile"),
            }
        }

        Self { excluded_app, ..Self::from_stored(&stored) }
    }

    fn from_stored(stored: &Stored) -> Self {
        let language = match read_choice(stored, keys::LANGUAGE) {
            // "auto" is a sentinel, not a language code.
            Some(code) if code != crate::types::AUTO_LANGUAGE => Some(LanguageCode(code)),
            _ => None,
        };

        let input_device = match read_choice(stored, keys::INPUT_DEVICE) {
            Some(id) if id != "default" && !id.is_empty() => Some(id),
            _ => None,
        };

        let capture_mode = match read_choice(stored, keys::CAPTURE_MODE).as_deref() {
            Some("instant") => CaptureMode::Instant,
            _ => CaptureMode::OnDemand,
        };

        // One line, because registry/reachability.rs finds a setting's consumer
        // by reading `let field = …keys::X` here.
        let filler_strength = crate::types::FillerStrength::from_stored(&read_choice(stored, keys::FILLER_STRENGTH).unwrap_or_default());

        // One line, for the same reachability reason as the one above.
        let fix_grammar = read_bool(stored, keys::FIX_GRAMMAR).unwrap_or(false);
        let ai_grammar = read_bool(stored, keys::AI_GRAMMAR).unwrap_or(false);
        let learn_corrections = read_bool(stored, keys::LEARN_CORRECTIONS).unwrap_or(true);

        // One line each, for the same reachability reason as the ones above.
        let preview_before_paste = read_bool(stored, keys::PREVIEW_BEFORE_PASTE).unwrap_or(false);
        let live_captions = read_bool(stored, keys::LIVE_CAPTIONS).unwrap_or(false);

        // One line, for the same reachability reason as the ones above.
        let accuracy = crate::types::AccuracyMode::from_stored(&read_choice(stored, keys::ACCURACY).unwrap_or_default());

        // One line, for the same reachability reason as the two above.
        let tone = crate::types::Tone::from_stored(&read_choice(stored, keys::TONE).unwrap_or_default());

        Self {
            model_id: read_choice(stored, keys::TRANSCRIPTION_MODEL)
                .unwrap_or_else(|| "large-v3-turbo-q5_0".to_string()),
            language,
            input_device,
            capture_mode,
            cancel_countdown_ms: read_number(stored, keys::CANCEL_COUNTDOWN_MS)
                .unwrap_or(3_000.0)
                .clamp(MIN_COUNTDOWN_MS as f64, MAX_COUNTDOWN_MS as f64)
                as u64,
            discard_on_escape: read_bool(stored, keys::DISCARD_ON_ESCAPE).unwrap_or(false),
            finalize_timeout_ms: read_number(stored, keys::FINALIZE_TIMEOUT_MS).unwrap_or(8_000.0)
                as u64,
            auto_paste: read_bool(stored, keys::AUTO_PASTE).unwrap_or(true),
            restore_clipboard: read_bool(stored, keys::RESTORE_CLIPBOARD).unwrap_or(true),
            paste_delay_ms: read_number(stored, keys::PASTE_DELAY_MS).unwrap_or(40.0).max(0.0)
                as u64,
            clipboard_restore_delay_ms: read_number(stored, keys::CLIPBOARD_RESTORE_DELAY_MS)
                .unwrap_or(500.0)
                .max(0.0) as u64,
            strip_fillers: read_bool(stored, keys::STRIP_FILLERS).unwrap_or(false),
            filler_strength,
            spoken_commands: read_bool(stored, keys::SPOKEN_COMMANDS).unwrap_or(true),
            apply_corrections: read_bool(stored, keys::APPLY_CORRECTIONS).unwrap_or(false),
            fix_grammar,
            ai_grammar,
            learn_corrections,
            tone,
            preview_before_paste,
            preview_ms: read_number(stored, keys::PREVIEW_MS)
                .unwrap_or(1_500.0)
                .clamp(MIN_PREVIEW_MS as f64, MAX_PREVIEW_MS as f64) as u64,
            live_captions,
            normalise_punctuation: read_bool(stored, keys::NORMALISE_PUNCTUATION).unwrap_or(true),
            capitalise_sentences: read_bool(stored, keys::CAPITALISE_SENTENCES).unwrap_or(true),
            audio_feedback: read_bool(stored, keys::AUDIO_FEEDBACK).unwrap_or(true),
            quiet_speakers: read_bool(stored, keys::QUIET_SPEAKERS).unwrap_or(true),
            context_prompt: read_choice(stored, keys::CONTEXT_PROMPT)
                .map(|text| text.trim().to_string())
                .filter(|text| !text.is_empty()),
            accuracy,
            excluded_app: None,
        }
    }
}

type Stored = std::collections::HashMap<String, SettingValue>;

/**
 * WHAT:  A stored value if present and the right shape, otherwise the
 *        registry's declared default.
 * WHY:   Falling back to the DECLARATION rather than to a literal means the
 *        default lives in exactly one place — the registry — and adding a
 *        setting never requires remembering to mirror its default here.
 */
fn effective_setting(stored: &Stored, key: &str) -> Option<SettingValue> {
    stored
        .get(key)
        .cloned()
        .or_else(|| registry::setting_def(key).map(|def| def.default.clone()))
}

fn read_bool(stored: &Stored, key: &str) -> Option<bool> {
    match effective_setting(stored, key)? {
        SettingValue::Bool(value) => Some(value),
        _ => None,
    }
}

fn read_number(stored: &Stored, key: &str) -> Option<f64> {
    match effective_setting(stored, key)? {
        SettingValue::Number(value) => Some(value),
        _ => None,
    }
}

fn read_choice(stored: &Stored, key: &str) -> Option<String> {
    match effective_setting(stored, key)? {
        SettingValue::Choice(value) | SettingValue::Text(value) => Some(value),
        _ => None,
    }
}

impl SessionSettings {
    /// Pure registry defaults, with no database read at all.
    pub fn defaults() -> Self {
        Self::from_stored(&Stored::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty() -> Stored {
        Stored::new()
    }

    #[test]
    fn an_unwritten_setting_resolves_to_its_registry_default() {
        // A fresh install has no rows at all, so this is the normal path.
        let stored = empty();
        assert_eq!(read_bool(&stored, keys::AUTO_PASTE), Some(true));
        assert_eq!(read_bool(&stored, keys::STRIP_FILLERS), Some(false));
        assert_eq!(read_number(&stored, keys::CANCEL_COUNTDOWN_MS), Some(3000.0));
    }

    #[test]
    fn a_stored_value_beats_the_default() {
        let mut stored = empty();
        stored.insert(keys::AUTO_PASTE.to_string(), SettingValue::Bool(false));
        assert_eq!(read_bool(&stored, keys::AUTO_PASTE), Some(false));
    }

    #[test]
    fn a_value_of_the_wrong_shape_is_ignored_rather_than_crashing() {
        // Written by an older build, or corrupted. The caller's own fallback
        // takes over rather than the app refusing to record.
        let mut stored = empty();
        stored.insert(
            keys::AUTO_PASTE.to_string(),
            SettingValue::Text("yes".into()),
        );
        assert_eq!(read_bool(&stored, keys::AUTO_PASTE), None);
    }

    /**
     * SOURCE OF TRUTH KEYWORDS: paste_delays_reach_session_settings
     * WHAT:  Both paste delays load into SessionSettings, defaulted and stored.
     * WHY:   These two were declared, rendered, saved and read by nothing for
     *        as long as they have existed — the injector held its own timing
     *        and the only route in was a builder no code called. This asserts
     *        the first half of the route that replaced it; the second half —
     *        that the adapter uses what it is handed — is
     *        `the_paste_timing_comes_from_the_request`.
     */
    #[test]
    fn the_paste_delays_load_into_the_session_settings() {
        let defaults = SessionSettings::from_stored(&empty());
        assert_eq!(defaults.paste_delay_ms, 40);
        assert_eq!(defaults.clipboard_restore_delay_ms, 500);

        let mut stored = empty();
        stored.insert(keys::PASTE_DELAY_MS.to_string(), SettingValue::Number(120.0));
        stored.insert(
            keys::CLIPBOARD_RESTORE_DELAY_MS.to_string(),
            SettingValue::Number(400.0),
        );

        let tuned = SessionSettings::from_stored(&stored);
        assert_eq!(tuned.paste_delay_ms, 120);
        assert_eq!(tuned.clipboard_restore_delay_ms, 400);
    }

    /**
     * WHAT:  A negative delay cannot become an enormous one.
     * WHY:   `as u64` on a negative f64 saturates to 0 in Rust, but relying on
     *        that is relying on a cast's edge case. A hand-edited database with
     *        -1 must mean "no delay", never 18 quintillion milliseconds.
     */
    #[test]
    fn a_negative_paste_delay_clamps_to_zero() {
        let mut stored = empty();
        stored.insert(keys::PASTE_DELAY_MS.to_string(), SettingValue::Number(-5.0));
        assert_eq!(SessionSettings::from_stored(&stored).paste_delay_ms, 0);
    }

    #[test]
    fn a_blank_context_prompt_is_no_context_prompt() {
        assert_eq!(SessionSettings::from_stored(&empty()).context_prompt, None);

        let mut stored = empty();
        stored.insert(keys::CONTEXT_PROMPT.to_string(), SettingValue::Text("  \n ".into()));
        assert_eq!(SessionSettings::from_stored(&stored).context_prompt, None);

        stored.insert(
            keys::CONTEXT_PROMPT.to_string(),
            SettingValue::Text("  Rust, Tauri ".into()),
        );
        assert_eq!(
            SessionSettings::from_stored(&stored).context_prompt.as_deref(),
            Some("Rust, Tauri")
        );
    }

    #[test]
    fn an_unknown_key_has_no_default_and_no_value() {
        assert_eq!(read_bool(&empty(), "not.a.setting"), None);
    }

    #[test]
    fn the_auto_language_sentinel_is_not_treated_as_a_language_code() {
        // "auto" must become None, or we would pin a language literally called
        // "auto" and get nothing back from the engine.
        let mut stored = empty();
        stored.insert(
            keys::LANGUAGE.to_string(),
            SettingValue::Choice("auto".into()),
        );
        let resolved = match read_choice(&stored, keys::LANGUAGE) {
            Some(code) if code != crate::types::AUTO_LANGUAGE => Some(LanguageCode(code)),
            _ => None,
        };
        assert!(resolved.is_none());
    }

    fn profile_for(bundle_id: &str, excluded: bool, enabled: bool) -> crate::types::AppProfile {
        crate::types::AppProfile {
            bundle_id: bundle_id.into(),
            display_name: "Vault".into(),
            overrides: Default::default(),
            enabled,
            excluded,
        }
    }

    #[test]
    fn an_excluded_apps_profile_marks_the_session_excluded() {
        let db = Database::open_in_memory().expect("db");
        services::profiles::upsert_profile(&db, &profile_for("com.vault", true, true)).expect("saved");

        let there = SessionSettings::load_for_app(&db, Some("com.vault"));
        assert_eq!(there.excluded_app.as_deref(), Some("Vault"));

        // Every other app, and an unidentifiable one, is unaffected.
        assert!(SessionSettings::load_for_app(&db, Some("com.other")).excluded_app.is_none());
        assert!(SessionSettings::load_for_app(&db, None).excluded_app.is_none());
    }

    #[test]
    fn a_switched_off_profile_does_not_exclude_anything() {
        let db = Database::open_in_memory().expect("db");
        services::profiles::upsert_profile(&db, &profile_for("com.vault", true, false)).expect("saved");
        assert!(SessionSettings::load_for_app(&db, Some("com.vault")).excluded_app.is_none());
    }
}
