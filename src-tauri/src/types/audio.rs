/*!
 * SOURCE OF TRUTH KEYWORDS: AudioChunk, AudioLevel, CaptureMode, DeviceInfo,
 *   TARGET_SAMPLE_RATE, TARGET_CHANNELS, ChunkKind, MicTestUpdate,
 *   MicTestStopReason
 * WHAT:  The audio vocabulary: the format everything downstream assumes, a
 *        decoded chunk, the meter value the pill draws, and the capture mode.
 * WHY:   TARGET_SAMPLE_RATE is a constant rather than a parameter because
 *        Whisper does not accept anything else — making it configurable would
 *        only create a way to be wrong. Device rate is read at runtime and
 *        resampled to it; never assume the device gives you 16kHz, because
 *        Bluetooth headsets in particular renegotiate to odd rates mid-session.
 * WHERE: Produced by adapters/cpal and pipeline/capture.rs, consumed by
 *        pipeline/chunker.rs and the transcription engine.
 */

use serde::{Deserialize, Serialize};
use specta::Type;

/// Whisper's required input rate. Not configurable — see the module WHY.
pub const TARGET_SAMPLE_RATE: u32 = 16_000;
/// Whisper takes mono. Multi-channel input is downmixed by averaging.
pub const TARGET_CHANNELS: u16 = 1;

/**
 * SOURCE OF TRUTH KEYWORDS: AudioChunk, ChunkKind
 * WHAT:  A span of 16kHz mono f32 samples, tagged with where it sits in the
 *        session and whether it is the trailing fragment.
 * WHY:   The tail is marked explicitly because it is the only chunk on the
 *        latency-critical path — the engine shrinks its encoder context for it
 *        and keeps the full context for background chunks, where latency is
 *        hidden anyway. A boolean here is what lets that decision live in the
 *        adapter instead of leaking into the caller.
 * WHERE: Emitted by pipeline/chunker.rs; consumed by the TranscriptionEngine.
 */
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub samples: Vec<f32>,
    pub start_ms: u64,
    pub end_ms: u64,
    pub kind: ChunkKind,
}

impl AudioChunk {
    pub fn duration_ms(&self) -> u64 {
        self.end_ms.saturating_sub(self.start_ms)
    }

    pub fn duration_secs(&self) -> f32 {
        self.samples.len() as f32 / TARGET_SAMPLE_RATE as f32
    }

    pub fn is_tail(&self) -> bool {
        matches!(self.kind, ChunkKind::Tail)
    }

    /**
     * SOURCE OF TRUTH KEYWORDS: AudioChunk::barrier, is_barrier, tail_barrier
     * WHAT:  A Tail that carries no audio, positioned at `at_ms`.
     * WHY:   The ASR queue is FIFO, so a Tail submitted after every interior
     *        chunk is the only signal that those chunks have all been decoded.
     *        When a recording stops in a pause there is no audio left to decode,
     *        and finishing it on the spot raced the interior chunk still in the
     *        queue: its words arrived after the recording was delivered, were
     *        dropped as belonging to no session, and the person got nothing
     *        pasted. The barrier closes that race without decoding anything.
     * WHERE: Built by session/actor.rs::hand_off_to_delivery; recognised by
     *        pipeline/worker.rs, which answers it without calling the engine.
     */
    pub fn barrier(at_ms: u64) -> Self {
        Self { samples: Vec::new(), start_ms: at_ms, end_ms: at_ms, kind: ChunkKind::Tail }
    }

    pub fn is_barrier(&self) -> bool {
        self.is_tail() && self.samples.is_empty()
    }

    /// Whether the engine may shrink its encoder context for this decode. Every
    /// kind does: the tail to be fast when the user is waiting, an interim
    /// snapshot to be cheap while they talk, and an interior chunk so that a
    /// stop pressed during its background decode does not queue the tail behind
    /// a 2s full-context pass (measured). `+ AUDIO_CTX_HEADROOM` keeps the
    /// context wider than the audio, which is what protects accuracy.
    pub fn shrinks_context(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkKind {
    /// Closed at a VAD silence boundary while the user is still talking.
    /// Decoded in the background; its latency is invisible.
    Interior,
    /// The fragment left when stop was pressed. The only latency-critical one.
    Tail,
    /// A snapshot of the open chunk, decoded only to draw live captions. It
    /// never closes the chunk and never enters the transcript — see
    /// session/captions.rs.
    Interim,
}

/**
 * SOURCE OF TRUTH KEYWORDS: AudioLevel
 * WHAT:  A windowed loudness sample for the pill's waveform.
 * WHY:   Computed on the drain thread rather than shipped as raw samples — the
 *        pill must not receive 16,000 floats a second, and it has a hard 60fps
 *        budget while inference is running.
 * WHERE: Emitted on the audio:level event; drawn by the Waveform component.
 */
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
pub struct AudioLevel {
    pub rms: f32,
    pub peak: f32,
}

/**
 * SOURCE OF TRUTH KEYWORDS: MicTestUpdate, MicTestStopReason
 * WHAT:  What the Settings microphone check pushes: a level, or the end of the
 *        check and why it ended.
 * WHY:   One enum on one event rather than a level event plus a stop event, so
 *        the meter cannot receive a level after the stop that ended it. The
 *        reason is carried because the three endings need different words — a
 *        timeout is silent housekeeping, a lost device needs saying.
 * WHERE: Emitted by pipeline/mic_monitor.rs through the EventSink port; drawn
 *        by the MicTestPanel in Settings.
 */
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MicTestUpdate {
    Level { level: AudioLevel },
    Stopped { reason: MicTestStopReason },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MicTestStopReason {
    /// The user, or a newer check, ended it.
    Requested,
    /// Nobody stopped it, so it stopped itself rather than leave the mic lit.
    TimedOut,
    /// The device went away mid-check.
    DeviceLost,
}

/**
 * SOURCE OF TRUTH KEYWORDS: CaptureMode
 * WHAT:  Whether the microphone stream is opened on demand or kept warm.
 * WHY:   A warm stream gives a true pre-roll so the first syllable is never
 *        clipped — but on macOS it also keeps the orange microphone indicator
 *        lit permanently, which is not an acceptable default for a background
 *        app. So it is the user's choice and it defaults to OnDemand.
 * WHERE: A registry setting; read by adapters/cpal when starting a session.
 */
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CaptureMode {
    /// Default. Stream opens on key-down, overlapping the user's reaction time.
    #[default]
    OnDemand,
    /// Stream stays open. True pre-roll, permanently lit mic indicator.
    Instant,
}


/**
 * SOURCE OF TRUTH KEYWORDS: DeviceInfo
 * WHAT:  One selectable input device as Settings lists it.
 * WHERE: Produced by the AudioSource port; rendered in Settings.
 */
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub sample_rate: u32,
    pub channels: u16,
}
