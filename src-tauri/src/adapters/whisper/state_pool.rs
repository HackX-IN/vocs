/*!
 * SOURCE OF TRUTH KEYWORDS: StatePool, StateLease, acquire, release, audio_ctx,
 *   remembered_audio_ctx
 * WHAT:  A reusable set of WhisperState scratch buffers, handed out one at a
 *        time and returned automatically, each with the encoder context its
 *        last decode ran at.
 * WHY:   WhisperState is per-inference working memory and creating one is not
 *        free, so it is reused rather than rebuilt per decode. A pool rather
 *        than a `thread_local!` because a thread_local cannot be scoped to an
 *        engine instance: two engines — a test, or a model switch that has not
 *        finished tearing the old one down — would share one slot and hand a
 *        state built from one model's weights to the other. There is exactly
 *        one ASR worker, so the lock never contends; it is here for
 *        correctness, not throughput.
 *        The lease returns on Drop because the paths that would otherwise leak
 *        a state are the error paths, and an engine that leaks its scratch
 *        space on every failed decode allocates a fresh one next time — a
 *        transient error quietly becoming a permanent slowdown.
 *
 *        The remembered context exists because whisper.cpp runs language
 *        detection at the PREVIOUS decode's encoder context (see priming.rs).
 *        Only the pool can know it: the state outlives any one call.
 * WHERE: Owned by adapters/whisper/engine.rs::LoadedModel; leased for the
 *        duration of one transcribe call.
 */

use std::ffi::c_int;

use parking_lot::Mutex;
use whisper_rs::{WhisperContext, WhisperState};

use crate::error::{AppError, AppResult, ErrorCode};

use super::params::FULL_AUDIO_CTX;

/// A state and the encoder context its last decode ran at.
type Pooled = (WhisperState, c_int);

pub struct StatePool {
    states: Mutex<Vec<Pooled>>,
}

impl StatePool {
    /// Seeded with the state `prepare` already built and warmed, so the first
    /// real transcription does not pay for one. Its context is assumed to be
    /// the full one: the worst case costs a single priming decode, whereas
    /// assuming small could cost a full-size detection pass.
    pub fn seeded(state: WhisperState) -> Self {
        Self {
            states: Mutex::new(vec![(state, FULL_AUDIO_CTX)]),
        }
    }

    /**
     * WHAT:  Borrows a state, building one only if the pool is empty.
     * WHERE: adapters/whisper/engine.rs::transcribe.
     */
    pub fn acquire<'a>(&'a self, context: &WhisperContext) -> AppResult<StateLease<'a>> {
        let pooled = self.states.lock().pop();
        let (state, audio_ctx) = match pooled {
            Some(pooled) => pooled,
            None => (
                context.create_state().map_err(|err| {
                    AppError::new(
                        ErrorCode::EngineNotReady,
                        "Vocs could not prepare the transcription model for this recording.",
                    )
                    .with_detail(format!("{err:?}"))
                })?,
                FULL_AUDIO_CTX,
            ),
        };

        Ok(StateLease {
            pool: &self.states,
            state: Some(state),
            audio_ctx,
        })
    }
}

/**
 * SOURCE OF TRUTH KEYWORDS: StateLease
 * WHAT:  One borrowed WhisperState. Returns to the pool when dropped.
 * WHERE: Created by StatePool::acquire.
 */
pub struct StateLease<'a> {
    pool: &'a Mutex<Vec<Pooled>>,
    state: Option<WhisperState>,
    audio_ctx: c_int,
}

impl StateLease<'_> {
    pub fn get(&mut self) -> AppResult<&mut WhisperState> {
        self.state
            .as_mut()
            .ok_or_else(|| AppError::internal("whisper state lease was already released"))
    }

    /// The encoder context the state's last decode ran at.
    pub fn audio_ctx(&self) -> c_int {
        self.audio_ctx
    }

    /// Records the context a decode has just run at, so the next lease knows.
    pub fn note_audio_ctx(&mut self, audio_ctx: c_int) {
        self.audio_ctx = audio_ctx;
    }
}

impl Drop for StateLease<'_> {
    fn drop(&mut self) {
        if let Some(state) = self.state.take() {
            self.pool.lock().push((state, self.audio_ctx));
        }
    }
}
