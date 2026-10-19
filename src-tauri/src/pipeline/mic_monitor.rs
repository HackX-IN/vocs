/*!
 * SOURCE OF TRUTH KEYWORDS: MicMonitor, MIC_TEST_MAX_DURATION, MIC_TEST_FRAME,
 *   mic_test, Active, generation, level_throttle
 * WHAT:  Opens the microphone on request and streams throttled level readings
 *        to the Settings meter until it is stopped, replaced, or times out.
 * WHY:   A meter that only works while recording cannot answer the question it
 *        exists for — "is the right microphone picked, and is it loud enough?"
 *        — before the user has dictated anything. It goes through the same
 *        AudioSource port as a real recording, so what the meter hears is what
 *        dictation will hear, device selection included.
 *
 *        Three things keep it from becoming a way to leave the mic lit:
 *          - the monitor owns at most ONE stream; starting again replaces it;
 *          - a hard deadline closes it even if the window is closed or the
 *            frontend never calls stop, because the orange indicator on macOS
 *            is a privacy signal and a forgotten one is a bug;
 *          - the deadline task only ends a stream it still OWNS (the
 *            generation check), so a stale task cannot kill a newer check.
 *
 *        Levels are throttled to one frame per MIC_TEST_FRAME. The capture
 *        drain emits one per few milliseconds, which is right for a recording
 *        and wasteful for a bar that repaints at display rate.
 * WHERE: Owned by AppState; driven by ipc/commands/mic_test.rs; pushes through
 *        the EventSink port to the MicTestPanel in Settings.
 */

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::error::AppResult;
use crate::ports::audio::{AudioSource, CaptureConfig, CaptureEvent, CaptureSession};
use crate::ports::EventSink;
use crate::types::{AudioLevel, CaptureMode, MicTestStopReason, MicTestUpdate};

/// Longest a check may run unattended. Long enough to talk through a sound
/// check, short enough that a forgotten window does not hold the mic.
pub const MIC_TEST_MAX_DURATION: Duration = Duration::from_secs(60);

/// Minimum spacing between level events.
const MIC_TEST_FRAME: Duration = Duration::from_millis(33);

const EVENT_QUEUE_DEPTH: usize = 64;

struct Active {
    generation: u64,
    session: Box<dyn CaptureSession>,
}

#[derive(Default)]
struct Slot {
    next_generation: u64,
    active: Option<Active>,
}

#[derive(Clone)]
pub struct MicMonitor {
    audio: Arc<dyn AudioSource>,
    events: Arc<dyn EventSink>,
    slot: Arc<Mutex<Slot>>,
    max_duration: Duration,
}

impl MicMonitor {
    pub fn new(audio: Arc<dyn AudioSource>, events: Arc<dyn EventSink>) -> Self {
        Self::with_max_duration(audio, events, MIC_TEST_MAX_DURATION)
    }

    fn with_max_duration(
        audio: Arc<dyn AudioSource>,
        events: Arc<dyn EventSink>,
        max_duration: Duration,
    ) -> Self {
        Self {
            audio,
            events,
            slot: Arc::new(Mutex::new(Slot::default())),
            max_duration,
        }
    }

    /// Starts (or restarts) the check on `device_id`, None being the system
    /// default. Must be called inside the tokio runtime.
    pub fn start(&self, device_id: Option<String>) -> AppResult<()> {
        let (tx, rx) = mpsc::channel(EVENT_QUEUE_DEPTH);
        let config = CaptureConfig {
            device_id,
            // A check never wants the always-warm stream: it would outlive it.
            mode: CaptureMode::OnDemand,
        };

        // Close the old stream BEFORE opening the new one. Two streams on one
        // device is legal but doubles the work for a bar nobody can tell apart.
        self.release(None);
        let session = self.audio.start(&config, tx)?;

        // Swap under the lock and stop whatever it displaces: two overlapping
        // starts must not leave a stream running that nothing can reach.
        let (generation, displaced) = {
            let mut slot = self.slot.lock();
            slot.next_generation += 1;
            let generation = slot.next_generation;
            let displaced = slot.active.replace(Active { generation, session });
            (generation, displaced)
        };
        if let Some(old) = displaced {
            if let Err(err) = old.session.stop() {
                tracing::warn!(error = %err, "could not stop a displaced microphone check");
            }
        }

        let monitor = self.clone();
        tokio::spawn(async move { monitor.pump(generation, rx).await });
        Ok(())
    }

    /// Stops the check, if one is running. Idempotent.
    pub fn stop(&self) {
        if self.release(None) {
            self.events.mic_test_update(MicTestUpdate::Stopped {
                reason: MicTestStopReason::Requested,
            });
        }
    }

    /// Closes the active stream — only if `generation` matches, when given.
    /// Returns whether anything was closed.
    fn release(&self, generation: Option<u64>) -> bool {
        let taken = {
            let mut slot = self.slot.lock();
            let owned = match (&slot.active, generation) {
                (Some(active), Some(wanted)) => active.generation == wanted,
                _ => true,
            };
            if owned { slot.active.take() } else { None }
        };
        match taken {
            Some(active) => {
                if let Err(err) = active.session.stop() {
                    tracing::warn!(error = %err, "could not stop the microphone check cleanly");
                }
                true
            }
            None => false,
        }
    }

    /// Reads capture events until the deadline, a lost device, or the stream
    /// closing, forwarding throttled levels.
    async fn pump(&self, generation: u64, mut rx: mpsc::Receiver<CaptureEvent>) {
        let deadline = tokio::time::sleep(self.max_duration);
        tokio::pin!(deadline);

        let mut last_emit: Option<Instant> = None;
        // Peak is held between frames so a transient that lands in a dropped
        // frame still shows; rms is the latest reading.
        let mut held_peak = 0.0_f32;

        let reason = loop {
            tokio::select! {
                () = &mut deadline => break MicTestStopReason::TimedOut,
                event = rx.recv() => match event {
                    Some(CaptureEvent::Level(level)) => {
                        held_peak = held_peak.max(level.peak);
                        let due = last_emit.is_none_or(|at| at.elapsed() >= MIC_TEST_FRAME);
                        if due {
                            last_emit = Some(Instant::now());
                            self.events.mic_test_update(MicTestUpdate::Level {
                                level: AudioLevel { rms: level.rms, peak: held_peak },
                            });
                            held_peak = 0.0;
                        }
                    }
                    Some(CaptureEvent::Samples(_)) => {}
                    Some(CaptureEvent::Lost(err)) => {
                        tracing::warn!(error = %err, "microphone check lost its device");
                        break MicTestStopReason::DeviceLost;
                    }
                    // The stream closed under us: someone called stop().
                    None => return,
                },
            }
        };

        // Only announce an ending we caused. If the slot has moved on, a newer
        // check owns the meter and a Stopped here would blank it.
        if self.release(Some(generation)) {
            self.events.mic_test_update(MicTestUpdate::Stopped { reason });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;
    use crate::types::DeviceInfo;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// What a fake capture hands the test so it can push events and see stops.
    struct FakeSession {
        info: DeviceInfo,
        stops: Arc<AtomicUsize>,
    }

    impl CaptureSession for FakeSession {
        fn device(&self) -> &DeviceInfo {
            &self.info
        }
        fn native_sample_rate(&self) -> u32 {
            48_000
        }
        fn stop(self: Box<Self>) -> AppResult<()> {
            self.stops.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeAudio {
        sinks: Mutex<Vec<mpsc::Sender<CaptureEvent>>>,
        stops: Arc<AtomicUsize>,
        fail: bool,
    }

    impl AudioSource for FakeAudio {
        fn list_devices(&self) -> AppResult<Vec<DeviceInfo>> {
            Ok(vec![])
        }
        fn default_device(&self) -> AppResult<DeviceInfo> {
            Err(AppError::internal("unused"))
        }
        fn start(
            &self,
            _config: &CaptureConfig,
            sink: mpsc::Sender<CaptureEvent>,
        ) -> AppResult<Box<dyn CaptureSession>> {
            if self.fail {
                return Err(AppError::internal("no device"));
            }
            self.sinks.lock().push(sink);
            Ok(Box::new(FakeSession {
                info: DeviceInfo {
                    id: "fake".into(),
                    name: "Fake".into(),
                    is_default: true,
                    sample_rate: 48_000,
                    channels: 1,
                },
                stops: Arc::clone(&self.stops),
            }))
        }
    }

    #[derive(Default)]
    struct Collected {
        updates: Mutex<Vec<MicTestUpdate>>,
    }

    impl EventSink for Collected {
        fn session_state_changed(&self, _state: &crate::types::SessionState) {}
        fn audio_level(&self, _level: AudioLevel) {}
        fn mic_test_update(&self, update: MicTestUpdate) {
            self.updates.lock().push(update);
        }
        fn transcript_delivered(&self, _words: u32, _delivery: crate::types::DeliveryKind) {}
        fn set_pill_visible(&self, _visible: bool) {}
        fn caption_changed(&self, _caption: Option<&crate::types::Caption>) {}
        fn download_progress(&self, _progress: crate::types::DownloadProgress) {}
        fn model_state_changed(
            &self,
            _model_id: crate::types::ModelId,
            _state: crate::types::ModelState,
        ) {
        }
        fn speak_status_changed(&self, _status: crate::types::SpeakStatus) {}
        fn set_cancel_key_active(&self, _active: bool) {}
    }

    fn rig(max: Duration) -> (MicMonitor, Arc<FakeAudio>, Arc<Collected>) {
        let audio = Arc::new(FakeAudio::default());
        let events = Arc::new(Collected::default());
        let monitor = MicMonitor::with_max_duration(audio.clone(), events.clone(), max);
        (monitor, audio, events)
    }

    fn level(rms: f32, peak: f32) -> CaptureEvent {
        CaptureEvent::Level(AudioLevel { rms, peak })
    }

    async fn settle() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    #[tokio::test]
    async fn levels_reach_the_meter_and_stop_ends_the_stream_once() {
        let (monitor, audio, events) = rig(Duration::from_secs(5));
        monitor.start(None).expect("starts");
        let sink = audio.sinks.lock()[0].clone();

        sink.send(level(0.1, 0.4)).await.expect("sent");
        settle().await;
        assert_eq!(
            events.updates.lock().first(),
            Some(&MicTestUpdate::Level { level: AudioLevel { rms: 0.1, peak: 0.4 } })
        );

        monitor.stop();
        monitor.stop(); // idempotent
        assert_eq!(audio.stops.load(Ordering::SeqCst), 1);
        let stopped = events
            .updates
            .lock()
            .iter()
            .filter(|u| matches!(u, MicTestUpdate::Stopped { .. }))
            .count();
        assert_eq!(stopped, 1, "exactly one Stopped, however often stop is called");
    }

    #[tokio::test]
    async fn frames_inside_the_throttle_window_are_folded_into_the_next_one() {
        let (monitor, audio, events) = rig(Duration::from_secs(5));
        monitor.start(None).expect("starts");
        let sink = audio.sinks.lock()[0].clone();

        sink.send(level(0.1, 0.1)).await.expect("sent");
        sink.send(level(0.2, 0.9)).await.expect("sent");
        sink.send(level(0.3, 0.2)).await.expect("sent");
        settle().await;

        let levels = events.updates.lock().len();
        assert_eq!(levels, 1, "three back-to-back frames emit one event");
        monitor.stop();
    }

    #[tokio::test]
    async fn an_unattended_check_closes_itself_at_the_deadline() {
        let (monitor, audio, events) = rig(Duration::from_millis(40));
        monitor.start(None).expect("starts");

        tokio::time::sleep(Duration::from_millis(150)).await;

        assert_eq!(audio.stops.load(Ordering::SeqCst), 1, "the mic is released");
        assert_eq!(
            events.updates.lock().last(),
            Some(&MicTestUpdate::Stopped { reason: MicTestStopReason::TimedOut })
        );
    }

    #[tokio::test]
    async fn a_lost_device_ends_the_check_and_says_why() {
        let (monitor, audio, events) = rig(Duration::from_secs(5));
        monitor.start(None).expect("starts");
        let sink = audio.sinks.lock()[0].clone();

        sink.send(CaptureEvent::Lost(AppError::internal("unplugged")))
            .await
            .expect("sent");
        settle().await;

        assert_eq!(
            events.updates.lock().last(),
            Some(&MicTestUpdate::Stopped { reason: MicTestStopReason::DeviceLost })
        );
        assert_eq!(audio.stops.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn restarting_replaces_the_stream_and_the_old_one_cannot_stop_the_new() {
        let (monitor, audio, events) = rig(Duration::from_secs(5));
        monitor.start(None).expect("first");
        let first = audio.sinks.lock()[0].clone();
        monitor.start(Some("other".into())).expect("second");
        assert_eq!(audio.stops.load(Ordering::SeqCst), 1, "the first stream was closed");

        // The first pump's channel is gone; a late Lost on it must be inert.
        let _ = first.send(CaptureEvent::Lost(AppError::internal("late"))).await;
        settle().await;
        assert!(
            !events.updates.lock().iter().any(|u| matches!(u, MicTestUpdate::Stopped { .. })),
            "a stale check must not announce a stop for the live one"
        );

        monitor.stop();
        assert_eq!(audio.stops.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_device_that_will_not_open_reports_the_error_and_holds_nothing() {
        let audio = Arc::new(FakeAudio { fail: true, ..FakeAudio::default() });
        let events = Arc::new(Collected::default());
        let monitor = MicMonitor::new(audio.clone(), events);
        assert!(monitor.start(None).is_err());
        monitor.stop(); // nothing to stop, and no panic
        assert_eq!(audio.stops.load(Ordering::SeqCst), 0);
    }
}
