/*!
 * SOURCE OF TRUTH KEYWORDS: CAPABILITIES, capabilities, capability,
 *   setting_def, default_settings, nav_items, hotkey_defs, all_settings,
 *   validate_registry, RegistrySnapshot
 * WHAT:  The single source of truth for what this app has. One table of
 *        capabilities, plus the lookups every other layer uses to read it.
 * WHY:   A feature is an entry here, never a new pattern. Adding one wires up
 *        its settings UI, its permission preflight, its nav placement, its
 *        hotkey and its metrics at once, because all five read this table
 *        instead of keeping their own copies. The rule that follows from it:
 *        if you are writing a `match` on a feature name anywhere outside this
 *        module, the branch belongs in the table instead.
 *
 *        Built through LazyLock rather than as a `const`, because the entries
 *        hold owned Strings and Vecs. That is deliberate — the same structs
 *        cross to TypeScript through specta unchanged, so the frontend reads
 *        the identical table rather than a hand-maintained mirror of it.
 * WHERE: Read by ipc/factory.rs (preflight), services/settings.rs (defaults and
 *        validation), the Settings view (control generation), the dashboard
 *        shell (nav) and the hotkey adapter (bindings).
 */

pub mod capability;
pub mod keys;
pub mod tone;

#[cfg(test)]
mod icons;

/// Asserts that every entry in this table is actually READ by something. Our
/// other guardrails check structure; this one checks reachability, which is the
/// gap every High finding in two independent audits fell through.
#[cfg(test)]
mod reachability;

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use specta::Type;

pub use capability::{
    Capability, CapabilityKey, HotkeyDef, MetricDef, NavDef, SettingDef, SettingSection,
};

use crate::ports::permissions::OsPermission;
use crate::types::settings::{ChoiceSource, SettingChoice};
use crate::types::{
    EngineFeature, HotkeyBinding, KeyModifier, LatencyStage, SettingKind, SettingValue,
};

/// Longest context a user may enter, in characters. Whisper keeps ~224 tokens of
/// prompt in total and the dictionary shares that budget, so a context that
/// could swallow it would silently push every dictionary term out.
pub const CONTEXT_PROMPT_MAX_LEN: u32 = 240;

/// The table. Everything else in this module is a view onto it.
pub static CAPABILITIES: LazyLock<Vec<Capability>> = LazyLock::new(build_capabilities);

// ── Small constructors, so the table below reads as data rather than noise ──

fn text(value: &str) -> String {
    value.to_string()
}

fn toggle(
    key: &str,
    label: &str,
    description: &str,
    section: SettingSection,
    default: bool,
) -> SettingDef {
    SettingDef {
        key: text(key),
        label: text(label),
        description: text(description),
        section,
        kind: SettingKind::Toggle,
        default: SettingValue::Bool(default),
        requires_engine: vec![],
        requires_permission: vec![],
        advanced: false,
    }
}

fn free_text(
    key: &str,
    label: &str,
    description: &str,
    section: SettingSection,
    (placeholder, max_len): (&str, u32),
) -> SettingDef {
    SettingDef {
        key: text(key),
        label: text(label),
        description: text(description),
        section,
        kind: SettingKind::Text {
            placeholder: Some(text(placeholder)),
            max_len: Some(max_len),
        },
        default: SettingValue::Text(String::new()),
        requires_engine: vec![],
        requires_permission: vec![],
        advanced: false,
    }
}

fn number(
    key: &str,
    label: &str,
    description: &str,
    section: SettingSection,
    (min, max, step): (f64, f64, f64),
    unit: Option<&str>,
    default: f64,
) -> SettingDef {
    SettingDef {
        key: text(key),
        label: text(label),
        description: text(description),
        section,
        kind: SettingKind::Number {
            min,
            max,
            step,
            unit: unit.map(text),
        },
        default: SettingValue::Number(default),
        requires_engine: vec![],
        requires_permission: vec![],
        advanced: false,
    }
}

fn choice(
    key: &str,
    label: &str,
    description: &str,
    section: SettingSection,
    options: &[(&str, &str, &str)],
    default: &str,
) -> SettingDef {
    SettingDef {
        key: text(key),
        label: text(label),
        description: text(description),
        section,
        kind: SettingKind::Choice {
            options: options
                .iter()
                .map(|(value, label, description)| SettingChoice {
                    value: text(value),
                    label: text(label),
                    description: if description.is_empty() {
                        None
                    } else {
                        Some(text(description))
                    },
                })
                .collect(),
        },
        default: SettingValue::Choice(text(default)),
        requires_engine: vec![],
        requires_permission: vec![],
        advanced: false,
    }
}

fn dynamic_choice(
    key: &str,
    label: &str,
    description: &str,
    section: SettingSection,
    source: ChoiceSource,
    default: &str,
) -> SettingDef {
    SettingDef {
        key: text(key),
        label: text(label),
        description: text(description),
        section,
        kind: SettingKind::DynamicChoice { source },
        default: SettingValue::Choice(text(default)),
        requires_engine: vec![],
        requires_permission: vec![],
        advanced: false,
    }
}

/// Marks a setting as depending on an OS grant. See SettingDef.
fn needs_permission(mut def: SettingDef, permission: OsPermission) -> SettingDef {
    def.requires_permission.push(permission);
    def
}

fn advanced(mut def: SettingDef) -> SettingDef {
    def.advanced = true;
    def
}

fn metric(stage: LatencyStage, label: &str, user_facing: bool) -> MetricDef {
    MetricDef {
        stage,
        label: text(label),
        user_facing,
        order: stage as u32,
    }
}

/// ⌥Space — the default dictation hotkey.
fn default_hotkey() -> HotkeyBinding {
    HotkeyBinding {
        modifiers: vec![KeyModifier::Option],
        key: text("Space"),
    }
}

fn copy_last_default_hotkey() -> HotkeyBinding {
    HotkeyBinding {
        modifiers: vec![KeyModifier::Control, KeyModifier::Option],
        key: text("KeyC"),
    }
}

// ── The table ────────────────────────────────────────────────────────────

fn build_capabilities() -> Vec<Capability> {
    vec![
        Capability {
            key: CapabilityKey::Dictation,
            name: text("Dictation"),
            description: text("Press a hotkey, talk, press it again — your words are pasted where you were typing."),
            requires: vec![OsPermission::Microphone],
            engine_needs: vec![EngineFeature::Streaming],
            nav: None,
            hotkey: Some(HotkeyDef {
                id: text("dictation.toggle"),
                label: text("Start / stop dictation"),
                default: default_hotkey(),
                setting_key: Some(text(keys::DICTATION_HOTKEY)),
            }),
            metrics: vec![
                metric(LatencyStage::HotkeyDispatch, "Hotkey", false),
                metric(LatencyStage::DeviceOpen, "Capture (microphone open)", true),
                metric(LatencyStage::ChunkDecode, "Background decode", false),
                metric(LatencyStage::TailDecode, "Recognition (final decode)", true),
                metric(LatencyStage::Assemble, "Assemble", false),
                metric(LatencyStage::Enhance, "Correction", true),
                metric(LatencyStage::TotalFinalize, "Stop to pasted", true),
            ],
            settings: vec![
                SettingDef {
                    key: text(keys::DICTATION_HOTKEY),
                    label: text("Dictation hotkey"),
                    description: text("The key you press anywhere to start and stop."),
                    section: SettingSection::Recording,
                    kind: SettingKind::Hotkey,
                    default: SettingValue::Hotkey(default_hotkey()),
                    requires_engine: vec![],
                    requires_permission: vec![],
                    advanced: false,
                },
                choice(
                    keys::DICTATION_MODE,
                    "Hotkey behaviour",
                    "Toggle presses once to start and once to stop. Push-to-talk records only while held.",
                    SettingSection::Recording,
                    &[
                        ("toggle", "Toggle", "Press to start, press again to stop"),
                        ("push_to_talk", "Push to talk", "Hold to record, release to send"),
                    ],
                    "toggle",
                ),
                choice(
                    keys::CAPTURE_MODE,
                    "Microphone",
                    "When Vocs opens the microphone. Opening it when you press the key keeps the orange indicator off except while you record. Keeping it warm never clips your first word, but macOS shows the indicator all the time.",
                    SettingSection::Recording,
                    &[
                        ("on_demand", "Open when I press the key", "Indicator only shows while recording"),
                        ("instant", "Keep it warm (instant)", "Never clips the first word; indicator always lit"),
                    ],
                    "on_demand",
                ),
                toggle(
                    keys::DISCARD_ON_ESCAPE,
                    "Escape discards immediately",
                    "Press Escape and the recording is gone at once, with no countdown to change your mind.",
                    SettingSection::Recording,
                    false,
                ),
                number(
                    keys::CANCEL_COUNTDOWN_MS,
                    "Cancel countdown",
                    "How long Escape waits before discarding a recording. Press Escape again during the countdown to keep it.",
                    SettingSection::Recording,
                    (1000.0, 10_000.0, 500.0),
                    Some("ms"),
                    3000.0,
                ),
                dynamic_choice(
                    keys::INPUT_DEVICE,
                    "Input device",
                    "Which microphone to record from.",
                    SettingSection::Recording,
                    ChoiceSource::InputDevices,
                    "default",
                ),
                toggle(
                    keys::AUDIO_FEEDBACK,
                    "Sounds",
                    "A short sound when recording starts and when text is delivered.",
                    SettingSection::Recording,
                    true,
                ),
                toggle(
                    keys::QUIET_SPEAKERS,
                    "Quiet the speakers while dictating",
                    "Mutes your Mac's built-in speakers while you record, so other people's voices in a meeting are not picked up by the microphone and typed out with yours. They come back the moment you stop. Does nothing with headphones, AirPods or an external speaker.",
                    SettingSection::Recording,
                    true,
                ),
                toggle(
                    keys::LIVE_CAPTIONS,
                    "Live captions",
                    "Show what Vocs hears, and what it will write, in a small window as you speak. It transcribes a little extra while you talk, so stopping can take a moment longer on a slow Mac.",
                    SettingSection::Recording,
                    false,
                ),
                advanced(number(
                    keys::FINALIZE_TIMEOUT_MS,
                    "Finalize timeout",
                    "If the last fragment has not decoded by this point, deliver what is ready rather than waiting.",
                    SettingSection::Recording,
                    (2000.0, 30_000.0, 500.0),
                    Some("ms"),
                    8000.0,
                )),
            ],
        },
        Capability {
            key: CapabilityKey::Models,
            name: text("Transcription"),
            description: text("The speech recognition model and the language it listens for."),
            requires: vec![],
            engine_needs: vec![],
            nav: None,
            hotkey: None,
            metrics: vec![],
            settings: vec![
                dynamic_choice(
                    keys::TRANSCRIPTION_MODEL,
                    "Model",
                    "Larger models are more accurate; smaller ones use less memory.",
                    SettingSection::Transcription,
                    ChoiceSource::Models,
                    "large-v3-turbo-q5_0",
                ),
                SettingDef {
                    key: text(keys::LANGUAGE),
                    label: text("Language"),
                    description: text("Pick your language to skip detection: each dictation finishes about a quarter of a second sooner and is a little more accurate. Leave it on Auto if you switch between languages."),
                    section: SettingSection::Transcription,
                    kind: SettingKind::DynamicChoice {
                        source: ChoiceSource::Languages,
                    },
                    default: SettingValue::Choice(text("auto")),
                    requires_engine: vec![EngineFeature::LanguageAutoDetect],
                    requires_permission: vec![],
                    advanced: false,
                },
                SettingDef {
                    requires_engine: vec![EngineFeature::InitialPrompt],
                    ..free_text(
                        keys::CONTEXT_PROMPT,
                        "Context",
                        "Tell the model what you usually talk about — jargon, product names, the topic. It listens for those words first. Your dictionary terms are added after it.",
                        SettingSection::Transcription,
                        ("e.g. Rust, Tauri, Kubernetes, Wire One Labs", CONTEXT_PROMPT_MAX_LEN),
                    )
                },
            ],
        },
        Capability {
            key: CapabilityKey::Settings,
            name: text("Output"),
            description: text("What happens to your words once they are transcribed."),
            // Accessibility is NOT required: without it delivery degrades to
            // clipboard-only, which is a supported outcome rather than a failure.
            requires: vec![],
            engine_needs: vec![],
            nav: Some(NavDef {
                label: text("Settings"),
                route: text("settings"),
                icon: text("Settings"),
                order: 30,
            }),
            hotkey: None,
            metrics: vec![
                metric(LatencyStage::ClipboardWrite, "Clipboard", false),
                metric(LatencyStage::Inject, "Output (paste)", true),
            ],
            settings: vec![
                needs_permission(
                    toggle(
                        keys::AUTO_PASTE,
                        "Paste automatically",
                        "Paste into whatever had focus. Turn this off to only copy to the clipboard.",
                        SettingSection::Output,
                        true,
                    ),
                    OsPermission::Accessibility,
                ),
                needs_permission(
                    toggle(
                        keys::RESTORE_CLIPBOARD,
                        "Restore my clipboard",
                        "Put back what was on the clipboard after pasting. Turn this off if pastes arrive empty in a particular app.",
                        SettingSection::Output,
                        true,
                    ),
                    OsPermission::Accessibility,
                ),
                toggle(
                    keys::COPY_LAST_ENABLED,
                    "Copy-last shortcut",
                    "Turn on a global shortcut that copies your most recent transcript to the clipboard — the quick way back when a paste landed in the wrong place.",
                    SettingSection::Output,
                    false,
                ),
                SettingDef {
                    key: text(keys::COPY_LAST_HOTKEY),
                    label: text("Copy-last shortcut keys"),
                    description: text("Use a key combination such as ⌃⌥C. A single modifier on its own cannot be used here."),
                    section: SettingSection::Output,
                    kind: SettingKind::Hotkey,
                    default: SettingValue::Hotkey(copy_last_default_hotkey()),
                    requires_engine: vec![],
                    requires_permission: vec![],
                    advanced: false,
                },
                toggle(
                    keys::PREVIEW_BEFORE_PASTE,
                    "Preview before pasting",
                    "Show the finished text for a moment before it is pasted. Press Escape to keep it out of the document; it is still saved in Dictations.",
                    SettingSection::Output,
                    false,
                ),
                number(
                    keys::PREVIEW_MS,
                    "Preview time",
                    "How long the preview waits before pasting. Only applies when previewing is on.",
                    SettingSection::Output,
                    (500.0, 5000.0, 100.0),
                    Some("ms"),
                    1500.0,
                ),
                advanced(number(
                    keys::PASTE_DELAY_MS,
                    "Paste delay",
                    "How long to wait after writing the clipboard before pasting. Raise it if an app pastes stale text.",
                    SettingSection::Output,
                    (0.0, 500.0, 5.0),
                    Some("ms"),
                    40.0,
                )),
                advanced(number(
                    keys::CLIPBOARD_RESTORE_DELAY_MS,
                    "Clipboard restore delay",
                    "How long to wait after pasting before restoring the clipboard.",
                    SettingSection::Output,
                    (0.0, 1000.0, 10.0),
                    Some("ms"),
                    150.0,
                )),
                toggle(
                    keys::CAPITALISE_SENTENCES,
                    "Capitalise sentences",
                    "Start each sentence with a capital letter.",
                    SettingSection::Writing,
                    true,
                ),
                toggle(
                    keys::NORMALISE_PUNCTUATION,
                    "Tidy punctuation",
                    "Normalise spacing, quotes and terminal punctuation.",
                    SettingSection::Writing,
                    true,
                ),
                toggle(
                    keys::STRIP_FILLERS,
                    "Remove filler words",
                    "Drop “um”, “uh” and “er”, and — depending on the level below — “like” and “you know” when you say them as filler. Words you mean, as in “I like it”, are never touched.",
                    SettingSection::Writing,
                    false,
                ),
                choice(
                    keys::FILLER_STRENGTH,
                    "How much filler to remove",
                    "Only applies when filler removal is on. Higher levels can change what you said, so start light.",
                    SettingSection::Writing,
                    &[
                        ("light", "Light", "Just “um”, “uh” and “er”"),
                        ("standard", "Standard", "Plus “like” and “you know” when you say them as filler"),
                        ("strict", "Strict", "Also “actually” and “basically” between commas"),
                    ],
                    "standard",
                ),
                choice(
                    keys::TONE,
                    "Writing style",
                    "How formally your words are written: casual for chat, formal for email, literal for a terminal or code editor.",
                    SettingSection::Writing,
                    &[
                        ("natural", "Natural", "As you said it, tidied"),
                        ("casual", "Casual", "Chat: contractions stay, and a one-line message gets no full stop"),
                        ("formal", "Formal", "Email: “don’t” becomes “do not”, “gonna” becomes “going to”"),
                        ("literal", "Literal", "Exactly what was heard, for a terminal or code editor — only your dictionary applies"),
                    ],
                    "natural",
                ),
                toggle(
                    keys::SPOKEN_COMMANDS,
                    "Spoken formatting",
                    "Turn “new line”, “new paragraph”, “comma” and “period” into the thing you said.",
                    SettingSection::Writing,
                    true,
                ),
                toggle(
                    keys::APPLY_CORRECTIONS,
                    "Apply spoken corrections",
                    "When you correct yourself out loud — \u{201c}Tuesday, sorry, I meant Wednesday\u{201d} — keep only the correction. It matches spoken phrases like \u{201c}I meant\u{201d} and \u{201c}make that\u{201d}; it does not rewrite your wording.",
                    SettingSection::Writing,
                    false,
                ),
                toggle(
                    keys::FIX_GRAMMAR,
                    "Fix common grammar slips",
                    "Corrects a small set of slips that are wrong every time \u{2014} \u{201c}a idea\u{201d} to \u{201c}an idea\u{201d}, \u{201c}she don\u{2019}t\u{201d} to \u{201c}she doesn\u{2019}t\u{201d}, \u{201c}we was\u{201d} to \u{201c}we were\u{201d}. It never rewrites your sentences, skips quoted speech and your dictionary words, and History shows every change. English only.",
                    SettingSection::Grammar,
                    false,
                ),
                toggle(
                    keys::AI_GRAMMAR,
                    "Smart grammar (on-device AI)",
                    "Runs a small grammar model on your Mac after the rules, to fix agreement, tense and wrong word forms the rules cannot see. It only keeps safe word-level fixes \u{2014} never names, numbers, code or your dictionary words \u{2014} and History shows every change. Needs a one-time download of about 120 MB below, then works offline. English only; adds roughly a tenth of a second for the last sentence.",
                    SettingSection::SmartGrammar,
                    false,
                ),
                toggle(
                    keys::LEARN_CORRECTIONS,
                    "Learn from my corrections",
                    "When Smart grammar fixes something and you leave it, Vocs remembers. After you have kept the same fix three times and never undone it, Vocs applies it itself, instantly and without the model. \u{201c}Use original\u{201d} or editing the fix away counts as a no and switches it off. Everything it learns is listed below, where you can turn any of it off or delete it.",
                    SettingSection::SmartGrammar,
                    true,
                ),
                toggle(
                    keys::LAUNCH_AT_LOGIN,
                    "Launch at login",
                    "Start Vocs automatically when you log in.",
                    SettingSection::General,
                    false,
                ),
                number(
                    keys::BASELINE_WPM,
                    "Your typing speed",
                    "Used to work out how much time dictation has saved you.",
                    SettingSection::General,
                    (10.0, 200.0, 1.0),
                    Some("wpm"),
                    40.0,
                ),
                toggle(
                    keys::CHECK_UPDATES,
                    "Check for updates",
                    "Check for a new version on launch and once a day. Apart from downloading models, this is the only network request Vocs makes.",
                    SettingSection::General,
                    true,
                ),
            ],
        },
        /*
         * SOURCE OF TRUTH KEYWORDS: Billing, Community, free_forever, open_source
         * WHAT:  A Community & Support page for the open-source project.
         * WHY:   Vocs is 100% local-first, free, and open source. This page shares
         *        the open-source manifesto and provides links for the community
         *        to star the repository, report issues, and sponsor the project.
         *
         *        Declares no settings and no metrics, which is the point: it is
         *        a nav entry and nothing else. That is legal here because the
         *        reachability test walks settings and metrics, and a capability
         *        with neither has nothing to be unreachable.
         * WHERE: Rendered from the registry by the dashboard shell; the page
         *        itself is a frontend component on the `billing` route.
         */
        Capability {
            key: CapabilityKey::Billing,
            name: text("Community"),
            description: text("Vocs is 100% free, private, and open source. Support the project."),
            requires: vec![],
            engine_needs: vec![],
            nav: Some(NavDef {
                label: text("Community"),
                // Last in the sidebar.
                route: text("billing"),
                icon: text("Heart"),
                order: 40,
            }),
            hotkey: None,
            metrics: vec![],
            settings: vec![],
        },
        Capability {
            key: CapabilityKey::History,
            name: text("History"),
            description: text("Everything you have dictated, searchable, on this machine only."),
            requires: vec![],
            engine_needs: vec![],
            nav: Some(NavDef {
                // "Dictations" in the sidebar and the page title; the capability
                // keeps its internal name. It is FIRST: it is the home of the
                // app — the notebook of everything you have said.
                label: text("Dictations"),
                route: text("history"),
                icon: text("NotebookText"),
                order: 10,
            }),
            hotkey: None,
            metrics: vec![],
            settings: vec![
                choice(
                    keys::RETENTION_DAYS,
                    "Keep history for",
                    "How long your dictations stay on this Mac. Anything older is deleted automatically in the background. Your dictionary and settings are never deleted.",
                    SettingSection::Privacy,
                    &[
                        ("0", "Forever", "Nothing is deleted until you delete it"),
                        ("7", "1 week", "Dictations older than a week are deleted"),
                        ("30", "1 month", "Dictations older than 30 days are deleted"),
                        ("90", "3 months", "Dictations older than 90 days are deleted"),
                        ("365", "1 year", "Dictations older than a year are deleted"),
                    ],
                    "0",
                ),
            ],
        },
        Capability {
            key: CapabilityKey::Stats,
            name: text("Stats"),
            description: text("How much you have dictated, and how fast Vocs actually is."),
            requires: vec![],
            engine_needs: vec![],
            nav: Some(NavDef {
                label: text("Stats"),
                route: text("stats"),
                icon: text("ChartNoAxesColumn"),
                order: 20,
            }),
            hotkey: None,
            metrics: vec![],
            settings: vec![],
        },
        Capability {
            key: CapabilityKey::Dictionary,
            name: text("Dictionary"),
            description: text("Names and jargon the model keeps getting wrong, and what they should be."),
            requires: vec![],
            // Prompting the engine with these terms is what fixes recognition
            // rather than patching the output, so the feature declares it.
            engine_needs: vec![EngineFeature::InitialPrompt],
            nav: None,
            hotkey: None,
            metrics: vec![],
            settings: vec![],
        },
        Capability {
            key: CapabilityKey::Onboarding,
            name: text("Setup"),
            description: text("First-run permissions, model download, and a hotkey test."),
            requires: vec![],
            engine_needs: vec![],
            nav: None,
            hotkey: None,
            metrics: vec![],
            settings: vec![toggle(
                keys::ONBOARDING_COMPLETE,
                "Setup complete",
                "Whether first-run setup has been finished.",
                SettingSection::General,
                false,
            )],
        },
        Capability {
            key: CapabilityKey::Updates,
            name: text("Updates"),
            description: text("Signed updates from GitHub Releases."),
            requires: vec![],
            engine_needs: vec![],
            nav: None,
            hotkey: None,
            metrics: vec![],
            settings: vec![],
        },
    ]
}

// ── Views onto the table ─────────────────────────────────────────────────

pub fn capabilities() -> &'static [Capability] {
    &CAPABILITIES
}

pub fn capability(key: CapabilityKey) -> Option<&'static Capability> {
    CAPABILITIES.iter().find(|c| c.key == key)
}

/// Every setting in the app, flattened across capabilities.
pub fn all_settings() -> impl Iterator<Item = &'static SettingDef> {
    CAPABILITIES.iter().flat_map(|c| c.settings.iter())
}

pub fn setting_def(key: &str) -> Option<&'static SettingDef> {
    all_settings().find(|s| s.key == key)
}

/// The defaults every fresh install starts from, and the fallback for any
/// setting that has never been written.
pub fn default_settings() -> HashMap<String, SettingValue> {
    all_settings()
        .map(|def| (def.key.clone(), def.default.clone()))
        .collect()
}

/// Sidebar entries, in declared order.
pub fn nav_items() -> Vec<&'static NavDef> {
    let mut items: Vec<&NavDef> = CAPABILITIES.iter().filter_map(|c| c.nav.as_ref()).collect();
    items.sort_by_key(|n| n.order);
    items
}

pub fn hotkey_defs() -> Vec<&'static HotkeyDef> {
    CAPABILITIES.iter().filter_map(|c| c.hotkey.as_ref()).collect()
}

/**
 * SOURCE OF TRUTH KEYWORDS: RegistrySnapshot
 * WHAT:  The whole table as one serialisable value for the frontend.
 * WHY:   The frontend reads the SAME declarations the backend enforces, rather
 *        than a mirror someone has to remember to update. That is what makes a
 *        generated settings page trustworthy.
 * WHERE: Returned by the registry IPC command; consumed by the Settings view
 *        and the dashboard shell.
 */
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RegistrySnapshot {
    pub capabilities: Vec<Capability>,
}

pub fn snapshot() -> RegistrySnapshot {
    RegistrySnapshot {
        capabilities: CAPABILITIES.clone(),
    }
}

/**
 * SOURCE OF TRUTH KEYWORDS: validate_registry
 * WHAT:  Self-check over the table: unique setting keys, unique capability
 *        keys, and every default matching its declared control kind.
 * WHY:   These are the mistakes that are invisible at compile time and silent
 *        at runtime — a duplicated key means one setting quietly shadows
 *        another, and a default of the wrong type means a control renders
 *        empty. Running this at startup turns both into a loud failure during
 *        development instead of a support ticket.
 * WHERE: Called once from lib.rs during setup, and asserted in tests.
 */
pub fn validate_registry() -> Result<(), Vec<String>> {
    let mut problems = Vec::new();
    let mut seen_settings: HashMap<&str, usize> = HashMap::new();
    let mut seen_capabilities: HashMap<CapabilityKey, usize> = HashMap::new();

    for capability in CAPABILITIES.iter() {
        *seen_capabilities.entry(capability.key).or_insert(0) += 1;

        for setting in &capability.settings {
            *seen_settings.entry(setting.key.as_str()).or_insert(0) += 1;

            if !setting.default.matches_kind(&setting.kind) {
                problems.push(format!(
                    "setting `{}` has a default that does not match its control kind",
                    setting.key
                ));
            }
        }
    }

    for (key, count) in seen_settings {
        if count > 1 {
            problems.push(format!("setting key `{key}` is declared {count} times"));
        }
    }
    for (key, count) in seen_capabilities {
        if count > 1 {
            problems.push(format!(
                "capability `{}` is declared {count} times",
                key.as_str()
            ));
        }
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_internally_consistent() {
        if let Err(problems) = validate_registry() {
            panic!("registry is invalid:\n  {}", problems.join("\n  "));
        }
    }

    #[test]
    fn every_declared_key_constant_resolves_to_a_setting() {
        // Guards against a constant being renamed without its declaration.
        for key in [
            keys::DICTATION_HOTKEY,
            keys::DICTATION_MODE,
            keys::CAPTURE_MODE,
            keys::CANCEL_COUNTDOWN_MS,
            keys::DISCARD_ON_ESCAPE,
            keys::INPUT_DEVICE,
            keys::AUDIO_FEEDBACK,
            keys::LIVE_CAPTIONS,
            keys::TRANSCRIPTION_MODEL,
            keys::LANGUAGE,
            keys::FINALIZE_TIMEOUT_MS,
            keys::CONTEXT_PROMPT,
            keys::AUTO_PASTE,
            keys::RESTORE_CLIPBOARD,
            keys::PASTE_DELAY_MS,
            keys::PREVIEW_BEFORE_PASTE,
            keys::PREVIEW_MS,
            keys::CLIPBOARD_RESTORE_DELAY_MS,
            keys::COPY_LAST_ENABLED,
            keys::COPY_LAST_HOTKEY,
            keys::CAPITALISE_SENTENCES,
            keys::NORMALISE_PUNCTUATION,
            keys::STRIP_FILLERS,
            keys::FILLER_STRENGTH,
            keys::SPOKEN_COMMANDS,
            keys::APPLY_CORRECTIONS,
            keys::FIX_GRAMMAR,
            keys::AI_GRAMMAR,
            keys::LEARN_CORRECTIONS,
            keys::TONE,
            keys::RETENTION_DAYS,
            keys::LAUNCH_AT_LOGIN,
            keys::BASELINE_WPM,
            keys::CHECK_UPDATES,
            keys::ONBOARDING_COMPLETE,
        ] {
            assert!(
                setting_def(key).is_some(),
                "setting constant `{key}` has no declaration in the registry"
            );
        }
    }

    #[test]
    fn validate_value_enforces_range_length_and_shape_for_every_path() {
        let delay = setting_def(keys::PASTE_DELAY_MS).expect("declared");
        assert!(delay.validate_value(&SettingValue::Number(40.0)).is_ok());
        assert!(delay.validate_value(&SettingValue::Number(501.0)).is_err());
        assert!(delay.validate_value(&SettingValue::Number(f64::NAN)).is_err());
        assert!(delay.validate_value(&SettingValue::Bool(true)).is_err());

        let context = setting_def(keys::CONTEXT_PROMPT).expect("declared");
        let limit = CONTEXT_PROMPT_MAX_LEN as usize;
        assert!(context.validate_value(&SettingValue::Text("a".repeat(limit))).is_ok());
        assert!(context.validate_value(&SettingValue::Text("a".repeat(limit + 1))).is_err());
    }

    #[test]
    fn the_user_facing_stages_run_in_pipeline_order() {
        let mut shown: Vec<(u32, &str)> = CAPABILITIES
            .iter()
            .flat_map(|c| c.metrics.iter())
            .filter(|m| m.user_facing)
            .map(|m| (m.order, m.label.as_str()))
            .collect();
        shown.sort_unstable();
        let labels: Vec<&str> = shown.into_iter().map(|(_, label)| label).collect();
        assert_eq!(
            labels,
            [
                "Capture (microphone open)",
                "Recognition (final decode)",
                "Correction",
                "Output (paste)",
                "Stop to pasted",
            ]
        );
    }

    #[test]
    fn nav_items_are_ordered() {
        let orders: Vec<u32> = nav_items().iter().map(|n| n.order).collect();
        let mut sorted = orders.clone();
        sorted.sort_unstable();
        assert_eq!(orders, sorted);
    }
}
