/*!
 * SOURCE OF TRUTH KEYWORDS: export_diagnostics, DiagnosticsReport, render_report,
 *   redact_home, describe_setting, read_log_tail, LOG_TAIL_BYTES
 * WHAT:  Builds a plain-text report a user can attach to a support request:
 *        versions, permissions, model state, settings, timings, recent failure
 *        codes and the tail of the log.
 * WHY:   The alternative is telemetry, which would break the product's one
 *        promise. A file the user creates, reads and chooses to send keeps
 *        support possible without the app ever phoning home.
 *
 *        So the report is built to be READ before it is sent, and to hold
 *        nothing that needs a second thought:
 *          - never a transcript, a dictionary term, a context prompt or an app's
 *            bundle id: free-text settings appear only as a character count,
 *            the dictionary and profiles only as counts, and the chosen input
 *            device only as "default" or "selected" (device names often carry
 *            the owner's name);
 *          - the home directory is replaced with "~" wherever it appears, since
 *            paths in errors carry the user's account name;
 *          - the log tail drops the one debug line that quotes dropped speech.
 *        What remains is state and numbers, which is what a bug report needs.
 * WHERE: Called by the Settings → Privacy diagnostics control, which saves the
 *        text through the same native dialog as every other export.
 */

use std::fmt::Write as _;
use std::path::Path;

use tauri::State;

use crate::error::AppError;
use crate::ipc::context::AppState;
use crate::ipc::factory::{execute, CommandSpec};
use crate::ports::permissions::OsPermission;
use crate::registry::{self, CapabilityKey};
use crate::services::{dictionary, metrics, profiles, sessions, settings};
use crate::telemetry::now_ms;
use crate::types::{ChoiceSource, LatencySummary, ModelState, SessionOutcome, SettingKind, SettingValue};

/// How much of the newest log file to include. Enough for a bad session; small
/// enough to read, and to attach.
const LOG_TAIL_BYTES: u64 = 48 * 1024;
const RECENT_FAILURES: i64 = 100;
const LATENCY_WINDOW: i64 = 200;
/// A log line quoting speech the model dropped; see the module WHY.
const SPEECH_LOG_MARKER: &str = "segment dropped as hallucination";

const EXPORT: CommandSpec = CommandSpec::new("export_diagnostics", CapabilityKey::Settings).reports();

/// Everything the report prints, already collected and already redacted.
#[derive(Debug, Default)]
struct DiagnosticsReport {
    app_version: String,
    os: String,
    arch: String,
    engine: String,
    model: String,
    permissions: Vec<(String, String)>,
    settings: Vec<(String, String)>,
    sessions: i64,
    dictionary_entries: usize,
    profiles: usize,
    latency: Vec<(String, f64, f64, i64)>,
    failures: Vec<(String, String)>,
    log_tail: String,
}

#[tauri::command]
#[specta::specta]
pub async fn export_diagnostics(state: State<'_, AppState>) -> Result<String, AppError> {
    execute(&state, EXPORT, (), |ctx, ()| async move {
        let db = ctx.db();
        let ports = ctx.ports();
        let home = dirs::home_dir();

        let mut report = DiagnosticsReport {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            os: operating_system(),
            arch: std::env::consts::ARCH.to_string(),
            engine: ports.engine.capabilities().id.as_str().to_string(),
            ..DiagnosticsReport::default()
        };

        for permission in [OsPermission::Microphone, OsPermission::Accessibility] {
            report
                .permissions
                .push((format!("{permission:?}"), format!("{:?}", ports.permissions.check(permission))));
        }

        let stored = settings::get_all_settings(db)?;
        report.model = match stored.get(registry::keys::TRANSCRIPTION_MODEL) {
            Some(SettingValue::Choice(id)) => id.clone(),
            _ => "default".to_string(),
        };
        let mut effective = registry::default_settings();
        effective.extend(stored);
        let mut rows: Vec<_> = registry::all_settings()
            .filter_map(|def| effective.get(&def.key).map(|value| (def.key.clone(), describe_setting(&def.kind, value))))
            .collect();
        rows.sort();
        report.settings = rows;

        for status in ports.models.list().await? {
            if matches!(status.state, ModelState::Ready) {
                report.model = format!("{} (ready)", status.descriptor.id.as_str());
            }
        }

        report.sessions = sessions::count_sessions(db)?;
        report.dictionary_entries = dictionary::list_entries(db)?.len();
        report.profiles = profiles::list_profiles(db)?.len();
        report.latency = metrics::latency_summary(db, LATENCY_WINDOW)?
            .into_iter()
            .map(|LatencySummary { stage, p50_ms, p95_ms, sample_count }| {
                (stage.as_str().to_string(), p50_ms, p95_ms, sample_count)
            })
            .collect();
        // Codes and the app's own sentences only — never the transcript.
        report.failures = sessions::list_sessions(db, &crate::types::HistoryFilter::default(), RECENT_FAILURES, 0)?
            .into_iter()
            .filter(|s| s.outcome == SessionOutcome::Failed)
            .map(|s| (s.error_code.unwrap_or_default(), s.error_message.unwrap_or_default()))
            .take(20)
            .collect();
        report.log_tail = read_log_tail(&ctx.paths().logs_dir);

        Ok(redact_home(&render_report(&report, now_ms()), home.as_deref()))
    })
    .await
}

/// One line of "key = value", with free text reduced to its length.
fn describe_setting(kind: &SettingKind, value: &SettingValue) -> String {
    match (kind, value) {
        // A device id is its name, and "Priya's AirPods" is a name.
        (SettingKind::DynamicChoice { source: ChoiceSource::InputDevices }, SettingValue::Choice(id)) => {
            if id == "default" { "default".to_string() } else { "selected".to_string() }
        }
        (SettingKind::Text { .. }, SettingValue::Text(text)) => format!("<{} characters, not included>", text.chars().count()),
        (_, SettingValue::Bool(on)) => on.to_string(),
        (_, SettingValue::Number(number)) => number.to_string(),
        (_, SettingValue::Choice(choice) | SettingValue::Text(choice)) => choice.clone(),
        (_, SettingValue::Hotkey(binding)) => binding.to_accelerator(),
    }
}

/// Replaces the user's home directory with "~". Longest-first is unnecessary:
/// the home path is matched whole.
fn redact_home(text: &str, home: Option<&Path>) -> String {
    match home.and_then(Path::to_str) {
        Some(home) if home.len() > 1 => text.replace(home, "~"),
        _ => text.to_string(),
    }
}

fn render_report(report: &DiagnosticsReport, now: i64) -> String {
    let mut out = String::new();
    let mut line = |text: String| {
        let _ = writeln!(out, "{text}");
    };

    line("Vocs diagnostics".into());
    line("This file was created on this Mac and has not been sent anywhere.".into());
    line("It contains no transcripts, no dictionary terms and no audio.".into());
    line(String::new());
    line(format!("Created (ms since epoch): {now}"));
    line(format!("Vocs {} · {} · {}", report.app_version, report.os, report.arch));
    line(format!("Engine: {} · Model: {}", report.engine, report.model));
    line(format!(
        "Sessions: {} · Dictionary entries: {} · App profiles: {}",
        report.sessions, report.dictionary_entries, report.profiles
    ));

    line(String::new());
    line("Permissions".into());
    for (name, state) in &report.permissions {
        line(format!("  {name}: {state}"));
    }

    line(String::new());
    line("Timings (p50 / p95 ms, samples)".into());
    if report.latency.is_empty() {
        line("  none recorded yet".into());
    }
    for (stage, p50, p95, samples) in &report.latency {
        line(format!("  {stage}: {p50:.0} / {p95:.0} ({samples})"));
    }

    line(String::new());
    line("Recent failures".into());
    if report.failures.is_empty() {
        line("  none".into());
    }
    for (code, message) in &report.failures {
        line(format!("  {code}: {message}"));
    }

    line(String::new());
    line("Settings".into());
    for (key, value) in &report.settings {
        line(format!("  {key} = {value}"));
    }

    line(String::new());
    line("Log (most recent lines)".into());
    line(report.log_tail.clone());
    out
}

/// The last LOG_TAIL_BYTES of the newest `vocs.log*` file, minus any line that
/// quotes speech. An unreadable or missing log is reported as such, not as an
/// error: a diagnostics export that fails is worse than one with a gap.
fn read_log_tail(dir: &Path) -> String {
    use std::io::{Read, Seek, SeekFrom};

    let newest = std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("vocs.log"))
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path);

    let Some(path) = newest else {
        return "  (no log file found)".to_string();
    };
    // Returns the text and whether the read started mid-file.
    let read = || -> std::io::Result<(String, bool)> {
        let mut file = std::fs::File::open(&path)?;
        let length = file.metadata()?.len();
        file.seek(SeekFrom::Start(length.saturating_sub(LOG_TAIL_BYTES)))?;
        let mut bytes = Vec::new();
        file.take(LOG_TAIL_BYTES).read_to_end(&mut bytes)?;
        Ok((String::from_utf8_lossy(&bytes).into_owned(), length > LOG_TAIL_BYTES))
    };

    match read() {
        Ok((text, cut_mid_file)) => {
            let mut lines: Vec<&str> = text.lines().filter(|l| !l.contains(SPEECH_LOG_MARKER)).collect();
            // Seeking into the middle of the file cuts the first line in half.
            if cut_mid_file && !lines.is_empty() {
                lines.remove(0);
            }
            lines.join("\n")
        }
        Err(_) => "  (the log could not be read)".to_string(),
    }
}

/// "macOS 14.5 (Build …)" from the system, or a plain label if it will not say.
fn operating_system() -> String {
    #[cfg(target_os = "macos")]
    {
        objc2_foundation::NSProcessInfo::processInfo()
            .operatingSystemVersionString()
            .to_string()
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::env::consts::OS.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::HotkeyBinding;

    fn text_kind() -> SettingKind {
        SettingKind::Text { placeholder: None, max_len: Some(100) }
    }

    #[test]
    fn free_text_settings_are_reduced_to_a_length() {
        let shown = describe_setting(&text_kind(), &SettingValue::Text("Project Nightingale, Acme Corp".into()));
        assert_eq!(shown, "<30 characters, not included>");
        assert!(!shown.contains("Nightingale"));
    }

    #[test]
    fn a_chosen_input_device_is_not_named() {
        let kind = SettingKind::DynamicChoice { source: ChoiceSource::InputDevices };
        assert_eq!(describe_setting(&kind, &SettingValue::Choice("default".into())), "default");
        assert_eq!(describe_setting(&kind, &SettingValue::Choice("Priya's AirPods".into())), "selected");
    }

    #[test]
    fn other_settings_are_shown_as_they_are() {
        assert_eq!(describe_setting(&SettingKind::Toggle, &SettingValue::Bool(true)), "true");
        assert_eq!(describe_setting(&SettingKind::Toggle, &SettingValue::Choice("standard".into())), "standard");
        let hotkey = HotkeyBinding { modifiers: vec![crate::types::KeyModifier::Option], key: "Space".into() };
        assert!(describe_setting(&SettingKind::Hotkey, &SettingValue::Hotkey(hotkey)).contains("Space"));
    }

    #[test]
    fn the_home_directory_never_survives_into_the_report() {
        let home = Path::new("/Users/someone");
        let redacted = redact_home("could not open /Users/someone/Library/x and /Users/someone/y", Some(home));
        assert_eq!(redacted, "could not open ~/Library/x and ~/y");
        assert!(!redacted.contains("someone"));
    }

    #[test]
    fn a_missing_or_trivial_home_changes_nothing() {
        assert_eq!(redact_home("/a/b", None), "/a/b");
        assert_eq!(redact_home("/a/b", Some(Path::new("/"))), "/a/b");
    }

    #[test]
    fn the_report_states_what_it_does_not_contain_and_lists_what_it_does() {
        let report = DiagnosticsReport {
            app_version: "0.1.0".into(),
            os: "macOS 14".into(),
            arch: "aarch64".into(),
            engine: "whisper".into(),
            model: "turbo (ready)".into(),
            permissions: vec![("Microphone".into(), "Granted".into())],
            settings: vec![("enhance.strip_fillers".into(), "true".into())],
            sessions: 7,
            dictionary_entries: 3,
            profiles: 1,
            latency: vec![("tail_decode".into(), 210.0, 340.0, 12)],
            failures: vec![("MICROPHONE_DENIED".into(), "denied".into())],
            log_tail: "line one".into(),
        };
        let text = render_report(&report, 1_700_000_000_000);
        for expected in ["no transcripts", "Vocs 0.1.0", "Microphone: Granted", "tail_decode: 210 / 340 (12)", "MICROPHONE_DENIED", "enhance.strip_fillers = true", "line one", "Dictionary entries: 3"] {
            assert!(text.contains(expected), "report is missing {expected:?}:\n{text}");
        }
    }

    #[test]
    fn an_empty_history_still_renders_a_complete_report() {
        let text = render_report(&DiagnosticsReport::default(), 0);
        assert!(text.contains("none recorded yet") && text.contains("Recent failures\n  none"));
    }

    #[test]
    fn the_log_tail_drops_lines_that_quote_speech_and_survives_a_missing_directory() {
        let dir = std::env::temp_dir().join(format!("vocs-diag-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        std::fs::write(
            dir.join("vocs.log.2026-10-18"),
            "INFO started\nDEBUG segment dropped as hallucination text=\"private words\"\nINFO done\n",
        )
        .expect("log written");

        let tail = read_log_tail(&dir);
        assert!(tail.contains("started") && tail.contains("done"));
        assert!(!tail.contains("private words"));

        let _ = std::fs::remove_dir_all(&dir);
        assert!(read_log_tail(&dir).contains("no log file"));
    }

    #[test]
    fn the_newest_log_file_is_the_one_read() {
        let dir = std::env::temp_dir().join(format!("vocs-diag-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        std::fs::write(dir.join("vocs.log.2026-10-17"), "OLD").expect("old log");
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(dir.join("vocs.log.2026-10-18"), "NEW").expect("new log");

        assert_eq!(read_log_tail(&dir), "NEW");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
