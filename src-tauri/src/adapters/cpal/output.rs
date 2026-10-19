/*!
 * SOURCE OF TRUTH KEYWORDS: OutputBridge, list_output_devices, find_output,
 *   is_virtual_name, fill_frames, VIRTUAL_HINTS
 * WHAT:  Opens a CoreAudio OUTPUT stream on a chosen device and plays mono f32
 *        audio into it, fed through a lock-free ring buffer.
 * WHY:   The mirror image of capture, with the same rule: the output callback
 *        runs on a realtime thread, so it may only copy samples out of a ring
 *        buffer — no allocating, locking or logging. Producers (speech, the
 *        passthrough pump) push from ordinary threads; a starved callback plays
 *        silence rather than stalling, because a gap in a meeting is better than
 *        a glitch that repeats.
 *
 *        Mono in, every channel out: speech and a microphone are mono, and a
 *        virtual device is usually stereo. The same sample goes to every
 *        channel, which is what a meeting app expects from a "microphone".
 *
 *        The bridge hands the producer BACK-PRESSURE (`push_blocking`) because
 *        speech renders faster than real time: a minute of it arrives in a
 *        second, and a ring that dropped the excess would cut the end of
 *        every long utterance.
 *
 *        Devices are named by their human name, as the input side does, and
 *        "default" follows the system default so swapping it needs no setting
 *        change. `is_virtual_name` is a heuristic over well-known loopback
 *        drivers — it only decides what Settings suggests, never what is allowed.
 * WHERE: Used by adapters/macos/speech (speaking) and adapters/cpal/passthrough.rs
 *        (safe mode); devices listed for the Voice settings.
 */

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig, SupportedStreamConfig};
use parking_lot::Mutex;
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};

use crate::error::{AppError, AppResult, ErrorCode};
use crate::types::AudioOutputDevice;

/// Ten seconds of mono audio at 48 kHz. Speech renders faster than real time, so
/// this is how far ahead of playback a producer may get before it has to wait.
const RING_FRAMES: usize = 48_000 * 10;

/// Names of the loopback drivers people route meetings through. Lowercase.
const VIRTUAL_HINTS: &[&str] =
    &["blackhole", "loopback", "soundflower", "vb-cable", "virtual", "teams audio", "zoomaudio", "voicemod", "ground control"];

/// Whether a device name looks like a virtual / loopback device.
pub fn is_virtual_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    VIRTUAL_HINTS.iter().any(|hint| lower.contains(hint))
}

fn device_name(device: &Device) -> Option<String> {
    device.description().ok().map(|d| d.name().to_string())
}

fn output_error(detail: impl std::fmt::Display) -> AppError {
    AppError::new(ErrorCode::SpeechFailed, "Vocs could not use that audio output.").with_detail(detail.to_string())
}

/// Every device that can play audio, with the loopback ones marked.
pub fn list_output_devices() -> AppResult<Vec<AudioOutputDevice>> {
    let host = cpal::default_host();
    let default_name = host.default_output_device().and_then(|d| device_name(&d));
    let mut devices = Vec::new();
    for device in host.output_devices().map_err(output_error)? {
        let Some(name) = device_name(&device) else {
            continue;
        };
        if device.default_output_config().is_err() {
            continue;
        }
        devices.push(AudioOutputDevice {
            id: name.clone(),
            is_virtual: is_virtual_name(&name),
            is_default: default_name.as_deref() == Some(name.as_str()),
            name,
        });
    }
    Ok(devices)
}

/// Finds an output by name; "default", empty or a vanished device is the system default.
pub fn find_output(device_id: Option<&str>) -> AppResult<(Device, SupportedStreamConfig)> {
    let host = cpal::default_host();
    let named = match device_id {
        None | Some("") | Some("default") => None,
        Some(name) => host
            .output_devices()
            .map_err(output_error)?
            .find(|d| device_name(d).as_deref() == Some(name)),
    };
    let device = named
        .or_else(|| host.default_output_device())
        .ok_or_else(|| AppError::new(ErrorCode::SpeechFailed, "Vocs could not find an audio output to speak into."))?;
    let config = device.default_output_config().map_err(output_error)?;
    Ok((device, config))
}

/**
 * WHAT:  Fills interleaved output frames from a source of mono samples, writing
 *        each sample to every channel and silence when the source runs dry.
 *        Returns how many real samples were used.
 * WHY:   Pulled out of the callback so the part that decides what the listener
 *        hears is testable without a sound card.
 */
pub fn fill_frames(data: &mut [f32], channels: usize, mut next: impl FnMut() -> Option<f32>) -> usize {
    let mut used = 0;
    for frame in data.chunks_mut(channels.max(1)) {
        let sample = match next() {
            Some(sample) => {
                used += 1;
                sample
            }
            None => 0.0,
        };
        frame.fill(sample);
    }
    used
}

pub struct OutputBridge {
    stream: Option<Stream>,
    producer: Mutex<HeapProd<f32>>,
    queued: Arc<AtomicUsize>,
    clear_requested: Arc<AtomicBool>,
    sample_rate: u32,
    device_name: String,
}

impl OutputBridge {
    /// Opens `device_id` (None: the system default) and starts playing silence.
    pub fn open(device_id: Option<&str>) -> AppResult<Self> {
        let (device, supported) = find_output(device_id)?;
        let name = device_name(&device).unwrap_or_else(|| "output".into());
        let sample_rate = supported.sample_rate();
        let channels = supported.channels() as usize;
        let format = supported.sample_format();
        let config: StreamConfig = supported.into();

        let (producer, mut consumer) = HeapRb::<f32>::new(RING_FRAMES).split();
        let queued = Arc::new(AtomicUsize::new(0));
        let clear_requested = Arc::new(AtomicBool::new(false));

        let (queued_cb, clear_cb) = (Arc::clone(&queued), Arc::clone(&clear_requested));
        let mut feed = move |data: &mut [f32]| {
            if clear_cb.swap(false, Ordering::AcqRel) {
                while consumer.try_pop().is_some() {}
                queued_cb.store(0, Ordering::Release);
            }
            let used = fill_frames(data, channels, || pop(&mut consumer));
            queued_cb.fetch_sub(used.min(queued_cb.load(Ordering::Relaxed)), Ordering::AcqRel);
        };

        let on_error = |err: cpal::Error| tracing::warn!(error = %err, "the audio output glitched");
        let stream = match format {
            SampleFormat::F32 => device.build_output_stream(config, move |data: &mut [f32], _| feed(data), on_error, None),
            SampleFormat::I16 => {
                // Stack scratch: no allocation on the realtime thread.
                const CHUNK: usize = 1024;
                device.build_output_stream(
                    config,
                    move |data: &mut [i16], _| {
                        let mut buffer = [0.0_f32; CHUNK];
                        // Whole frames per block, so a channel never straddles two.
                        let block_len = (CHUNK / channels.max(1)) * channels.max(1);
                        for block in data.chunks_mut(block_len.max(1)) {
                            feed(&mut buffer[..block.len()]);
                            for (out, sample) in block.iter_mut().zip(&buffer) {
                                *out = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
                            }
                        }
                    },
                    on_error,
                    None,
                )
            }
            other => return Err(output_error(format!("unsupported output sample format {other:?}"))),
        }
        .map_err(output_error)?;
        stream.play().map_err(output_error)?;

        Ok(Self {
            stream: Some(stream),
            producer: Mutex::new(producer),
            queued,
            clear_requested,
            sample_rate,
            device_name: name,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Frames waiting to play.
    pub fn queued_frames(&self) -> usize {
        self.queued.load(Ordering::Acquire)
    }

    /// Queues as much of `mono` as fits and returns how many frames it took.
    pub fn try_push(&self, mono: &[f32]) -> usize {
        let taken = self.producer.lock().push_slice(mono);
        self.queued.fetch_add(taken, Ordering::AcqRel);
        taken
    }

    /// Queues all of `mono`, waiting for room. False if `cancel` was raised first.
    pub fn push_blocking(&self, mono: &[f32], cancel: &AtomicBool) -> bool {
        let mut rest = mono;
        while !rest.is_empty() {
            if cancel.load(Ordering::Acquire) {
                return false;
            }
            let taken = self.try_push(rest);
            rest = &rest[taken..];
            if !rest.is_empty() {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        true
    }

    /// Waits until everything queued has played. False on cancel or timeout.
    pub fn wait_drained(&self, cancel: &AtomicBool, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while self.queued_frames() > 0 {
            if cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        // The device's own buffer still holds a few milliseconds.
        std::thread::sleep(Duration::from_millis(60));
        true
    }

    /// Drops everything queued, at the next callback.
    pub fn clear(&self) {
        self.clear_requested.store(true, Ordering::Release);
    }
}

fn pop(consumer: &mut HeapCons<f32>) -> Option<f32> {
    consumer.try_pop()
}

impl Drop for OutputBridge {
    fn drop(&mut self) {
        // Dropping the stream stops it; named so the order is on the page.
        self.stream.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn well_known_loopback_drivers_are_recognised_and_speakers_are_not() {
        for name in ["BlackHole 2ch", "Loopback Audio", "Soundflower (2ch)", "Microsoft Teams Audio", "VB-Cable"] {
            assert!(is_virtual_name(name), "{name} is a virtual device");
        }
        for name in ["MacBook Pro Speakers", "External Headphones", "AirPods Pro", "LG UltraFine Display Audio"] {
            assert!(!is_virtual_name(name), "{name} is real hardware");
        }
    }

    #[test]
    fn a_mono_sample_goes_to_every_channel_and_a_dry_source_is_silence() {
        let mut queue = vec![0.5_f32, -0.25].into_iter();
        let mut data = [9.0_f32; 8]; // four stereo frames
        let used = fill_frames(&mut data, 2, || queue.next());
        assert_eq!(used, 2);
        assert_eq!(data, [0.5, 0.5, -0.25, -0.25, 0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn a_source_that_never_runs_dry_is_fully_consumed() {
        let mut data = [0.0_f32; 12];
        assert_eq!(fill_frames(&mut data, 6, || Some(1.0)), 2);
        assert!(data.iter().all(|s| *s == 1.0));
    }
}
