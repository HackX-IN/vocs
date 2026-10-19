/*!
 * SOURCE OF TRUTH KEYWORDS: Passthrough, latency_action, MAX_LATENCY_FRAMES_MS,
 *   LatencyAction, meeting_safe_mode, raw_mic_bypass
 * WHAT:  Carries the microphone straight to an output device — in the meeting
 *        case, your own raw voice into the virtual device a call app listens to.
 * WHY:   This is Meeting Safe Mode's engine, and it is deliberately NOT built on
 *        the speech path: the moment safe mode matters is the moment speech just
 *        failed, so it must not depend on any piece of it. It opens its own
 *        input stream and its own output bridge and shares nothing with
 *        synthesis.
 *
 *        Two audio clocks are never exactly equal — a microphone at "48 kHz" and
 *        a virtual device at "48 kHz" drift apart by a few parts per thousand, so
 *        an unattended bridge between them either runs dry (gaps) or fills (an
 *        ever-growing delay, until what the meeting hears is seconds behind your
 *        lips). The pump therefore watches how much is queued and, past
 *        `MAX_LATENCY_MS`, jumps to live by dropping the backlog. A click once an
 *        hour beats a voice that slowly falls behind.
 *
 *        Using one device as both ends would feed the output straight back into
 *        the input, so it is refused.
 * WHERE: Started and stopped by adapters/macos/speech (the SpeechOutput port).
 */

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use cpal::Stream;
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::HeapRb;

use super::convert::RateConverter;
use super::output::OutputBridge;
use super::resample::downmix_to_mono;
use super::source::{build_stream, device_name, CpalAudioSource};
use crate::error::{AppError, AppResult, ErrorCode};

/// The longest the meeting may be behind the microphone before the backlog is dropped.
const MAX_LATENCY_MS: usize = 120;
/// How often the pump moves audio across.
const PUMP_INTERVAL: Duration = Duration::from_millis(5);
/// Two seconds of 48 kHz stereo between the realtime callback and the pump.
const INPUT_RING: usize = 48_000 * 2 * 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatencyAction {
    Push,
    /// The backlog is past the limit: drop it and carry on live.
    ClearThenPush,
}

/// What to do with fresh audio given how much is already queued.
pub fn latency_action(queued_frames: usize, sample_rate: u32) -> LatencyAction {
    let limit = sample_rate as usize * MAX_LATENCY_MS / 1000;
    if queued_frames > limit {
        LatencyAction::ClearThenPush
    } else {
        LatencyAction::Push
    }
}

pub struct Passthrough {
    input: Option<Stream>,
    stop: Arc<AtomicBool>,
    pump: Option<JoinHandle<()>>,
    input_name: String,
    output_name: String,
}

impl Passthrough {
    /// Starts carrying `input_id` (None: the default microphone) to `output_id`.
    pub fn start(input_id: Option<&str>, output_id: Option<&str>) -> AppResult<Self> {
        let (input_device, input_config) = CpalAudioSource::find_device(input_id)?;
        let input_name = device_name(&input_device).unwrap_or_else(|| "microphone".into());

        let bridge = OutputBridge::open(output_id)?;
        let output_name = bridge.device_name().to_string();
        if output_name == input_name {
            return Err(AppError::new(
                ErrorCode::SpeechFailed,
                "Pick an output that is not your microphone — passing a device to itself would feed back.",
            ));
        }

        let in_rate = input_config.sample_rate();
        let channels = input_config.channels() as usize;
        let format = input_config.sample_format();
        let out_rate = bridge.sample_rate();
        let mut converter = RateConverter::new(in_rate, out_rate)?;

        let (mut producer, mut consumer) = HeapRb::<f32>::new(INPUT_RING).split();
        let stream = build_stream(
            &input_device,
            &input_config.into(),
            format,
            move |samples: &[f32]| {
                // Realtime: copy and nothing else. A full ring drops the excess.
                let _ = producer.push_slice(samples);
            },
            |err| tracing::warn!(error = %err, "the passthrough microphone glitched"),
        )?;
        cpal::traits::StreamTrait::play(&stream).map_err(|err| {
            AppError::new(ErrorCode::SpeechFailed, "Vocs could not start your microphone.").with_detail(err.to_string())
        })?;

        let stop = Arc::new(AtomicBool::new(false));
        let pump_stop = Arc::clone(&stop);
        let pump = std::thread::Builder::new()
            .name("vocs-passthrough".into())
            .spawn(move || {
                let mut raw = vec![0.0_f32; 4096];
                let mut mono = Vec::with_capacity(4096);
                while !pump_stop.load(Ordering::Acquire) {
                    std::thread::sleep(PUMP_INTERVAL);
                    // Whole frames only, so a stereo pair is never split.
                    let available = consumer.occupied_len() / channels.max(1) * channels.max(1);
                    let take = available.min(raw.len() / channels.max(1) * channels.max(1));
                    if take == 0 {
                        continue;
                    }
                    let got = consumer.pop_slice(&mut raw[..take]);
                    mono.clear();
                    downmix_to_mono(&raw[..got], channels, &mut mono);
                    let Ok(converted) = converter.push(&mono) else {
                        continue;
                    };
                    if latency_action(bridge.queued_frames(), out_rate) == LatencyAction::ClearThenPush {
                        bridge.clear();
                    }
                    bridge.try_push(&converted);
                }
                // `bridge` drops here, closing the output stream.
            })
            .map_err(AppError::internal)?;

        tracing::info!(input = %input_name, output = %output_name, "meeting safe mode: microphone passed through");
        Ok(Self { input: Some(stream), stop, pump: Some(pump), input_name, output_name })
    }

    pub fn describe(&self) -> (&str, &str) {
        (&self.input_name, &self.output_name)
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.input.take();
        if let Some(pump) = self.pump.take() {
            let _ = pump.join();
        }
    }
}

impl Drop for Passthrough {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_small_backlog_just_plays() {
        assert_eq!(latency_action(0, 48_000), LatencyAction::Push);
        assert_eq!(latency_action(48_000 * 100 / 1000, 48_000), LatencyAction::Push);
    }

    #[test]
    fn a_backlog_past_the_limit_is_dropped_to_stay_live() {
        assert_eq!(latency_action(48_000 * 121 / 1000, 48_000), LatencyAction::ClearThenPush);
        assert_eq!(latency_action(48_000, 48_000), LatencyAction::ClearThenPush);
    }

    #[test]
    fn the_limit_scales_with_the_device_rate() {
        // 6,000 frames is 125 ms at 48 kHz but 136 ms at 44.1 kHz; both are over.
        assert_eq!(latency_action(6_000, 48_000), LatencyAction::ClearThenPush);
        assert_eq!(latency_action(5_000, 44_100), LatencyAction::Push);
    }
}
