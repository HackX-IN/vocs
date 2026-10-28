/*!
 * SOURCE OF TRUTH KEYWORDS: types, AudioChunk, SessionState, TranscriptSegment,
 *   EngineCapabilities, SettingValue, LatencyStage, DictionaryEntry
 * WHAT:  Barrel for every shared type in the backend.
 * WHY:   One import path, and one place to look before writing a new type. The
 *        rule that types live only here is what keeps a second, subtly
 *        different Session shape from appearing beside the first.
 * WHERE: Imported across the whole crate; mirrored to TypeScript by specta so
 *        the frontend never hand-writes an IPC type.
 */

pub mod audio;
pub mod backup;
pub mod caption;
pub mod dictionary;
pub mod diff;
pub mod engine;
pub mod language;
pub mod learned;
pub mod accuracy;
pub mod metrics;
pub mod numeric;
pub mod organize;
pub mod profile;
pub mod session;
pub mod settings;
pub mod transcript;

pub use accuracy::AccuracyMode;
pub use organize::OrganizeMode;
pub use audio::{
    AudioChunk, AudioLevel, CaptureMode, ChunkKind, DeviceInfo, MicTestStopReason, MicTestUpdate,
    TARGET_SAMPLE_RATE,
};
pub use caption::Caption;
pub use backup::{BackupDictionaryEntry, BackupFile, ImportSummary, BACKUP_FORMAT, BACKUP_VERSION};
pub use diff::{DiffKind, DiffSegment};
pub use learned::{CorrectionEdit, LearnedCorrection, LearnedId, AUTO_APPLY_AFTER};
pub use dictionary::{DictionaryEntry, DictionaryId, MatchKind};
pub use profile::AppProfile;
pub use language::{language_label, language_options, LanguageOption, AUTO_LANGUAGE};
pub use engine::{
    DownloadProgress, EngineCapabilities, EngineFeature, EngineId, LanguageSupport,
    ModelDescriptor, ModelId, ModelKind, ModelState,
};
pub use metrics::{
    ActivityDay, LanguageCount, LatencyStage, LatencySummary, MetricSample, StatsSummary,
};
pub use session::{
    DeliveryKind, HistoryFilter, HistoryStatus, RecordingMode, SessionId, SessionOutcome, SessionState,
    SessionSummary,
};
pub use settings::{
    ChoiceSource, FillerStrength, HotkeyBinding, Tone, KeyModifier, SettingChoice, SettingKind, SettingValue,
};
pub use transcript::{LanguageCode, LanguageHint, Transcript, TranscriptSegment};
