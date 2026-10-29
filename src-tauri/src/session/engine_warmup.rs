/*!
 * SOURCE OF TRUTH KEYWORDS: prepare_in_background, engine_warmup, ENGINE_PREPARING,
 *   first_run_engine_never_loaded, self_healing_engine
 * WHAT:  Starts loading the speech model on its own thread if the engine is not
 *        ready, and does nothing if it is ready or a load is already running.
 * WHY:   The engine used to be prepared in exactly one place — the warm-up at
 *        launch — and that warm-up skips a model that is not on disk yet, on the
 *        stated grounds that "onboarding will call prepare again". Nothing did.
 *        So on EVERY fresh install the sequence was: launch (no model, skip),
 *        onboarding downloads the model, the engine stays unloaded, and the Try
 *        step answers "Vocs is still starting up" for as long as the app runs —
 *        until it is quit and reopened, which nobody is told to do. It was
 *        invisible to us because anyone who has run Vocs before already has the
 *        model on disk.
 *
 *        Two callers close it: the model download (the moment a model becomes
 *        verified) and the recording start (a person pressing the hotkey on an
 *        engine that is not ready is the last chance to notice, and a retry from
 *        there heals a session whose load failed for a transient reason).
 *
 *        `prepare` blocks for well over a second, so it is never run on the
 *        caller's thread; a flag makes concurrent calls collapse into one load
 *        rather than two copies of the model in memory.
 * WHERE: ipc/commands/models.rs after a transcription model is downloaded;
 *        session/actor.rs when a recording is refused for an engine that is not ready.
 */

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::ports::TranscriptionEngine;

static ENGINE_PREPARING: AtomicBool = AtomicBool::new(false);

/// Returns true when the engine is (or is about to be) loading, false when it is
/// already ready or the load could not be started.
pub fn prepare_in_background(engine: Arc<dyn TranscriptionEngine>) -> bool {
    prepare_with(&ENGINE_PREPARING, engine)
}

/// The body of `prepare_in_background` over a caller-supplied flag, so a test can
/// use its own and never contend with another test for the process-wide one.
fn prepare_with(flag: &'static AtomicBool, engine: Arc<dyn TranscriptionEngine>) -> bool {
    if engine.is_ready() {
        return false;
    }
    if flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_err() {
        // A load is already running; that one will make the engine ready.
        return true;
    }

    let spawned = std::thread::Builder::new().name("vocs-engine-prepare".into()).spawn(move || {
        let started = std::time::Instant::now();
        match engine.prepare() {
            Ok(()) => tracing::info!(elapsed_ms = started.elapsed().as_millis() as u64, "engine prepared on demand"),
            Err(err) => tracing::warn!(error = %err, "the engine could not be prepared"),
        }
        flag.store(false, Ordering::Release);
    });

    if spawned.is_err() {
        flag.store(false, Ordering::Release);
        tracing::error!("could not start the engine preparation thread");
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;

    use crate::error::AppResult;
    use crate::ports::TranscribeRequest;
    use crate::types::{AudioChunk, EngineCapabilities, EngineId, LanguageSupport, TranscriptSegment};

    /// An engine whose load takes a moment and which counts how often it is asked.
    struct SlowToLoad {
        ready: AtomicBool,
        loads: AtomicUsize,
    }

    impl SlowToLoad {
        fn new() -> Arc<Self> {
            Arc::new(Self { ready: AtomicBool::new(false), loads: AtomicUsize::new(0) })
        }
    }

    impl TranscriptionEngine for SlowToLoad {
        fn capabilities(&self) -> EngineCapabilities {
            EngineCapabilities {
                id: EngineId("slow".into()),
                display_name: "Slow".into(),
                languages: LanguageSupport::All,
                features: vec![],
                realtime_factor: 1.0,
                requires_download: false,
                runs_offline: true,
            }
        }
        fn prepare(&self) -> AppResult<()> {
            self.loads.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(80));
            self.ready.store(true, Ordering::SeqCst);
            Ok(())
        }
        fn is_ready(&self) -> bool {
            self.ready.load(Ordering::SeqCst)
        }
        fn transcribe(&self, _: &AudioChunk, _: &TranscribeRequest) -> AppResult<Vec<TranscriptSegment>> {
            Ok(Vec::new())
        }
    }

    fn wait_until_ready(engine: &SlowToLoad) {
        for _ in 0..200 {
            if engine.is_ready() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("the engine never became ready");
    }

    #[test]
    fn an_unready_engine_is_loaded_in_the_background() {
        static FLAG: AtomicBool = AtomicBool::new(false);
        let engine = SlowToLoad::new();

        assert!(prepare_with(&FLAG, Arc::clone(&engine) as Arc<dyn TranscriptionEngine>));
        wait_until_ready(&engine);
        assert_eq!(engine.loads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn concurrent_requests_make_one_load_not_several() {
        static FLAG: AtomicBool = AtomicBool::new(false);
        let engine = SlowToLoad::new();

        for _ in 0..5 {
            assert!(prepare_with(&FLAG, Arc::clone(&engine) as Arc<dyn TranscriptionEngine>));
        }
        wait_until_ready(&engine);
        assert_eq!(engine.loads.load(Ordering::SeqCst), 1, "two copies of the model would double the memory");
    }

    #[test]
    fn a_ready_engine_is_left_alone() {
        static FLAG: AtomicBool = AtomicBool::new(false);
        let engine = SlowToLoad::new();
        engine.ready.store(true, Ordering::SeqCst);

        assert!(!prepare_with(&FLAG, Arc::clone(&engine) as Arc<dyn TranscriptionEngine>));
        assert_eq!(engine.loads.load(Ordering::SeqCst), 0);
    }
}
