/*!
 * SOURCE OF TRUTH KEYWORDS: ChunkBacklog, Deferred, MAX_BACKLOG, drain_into,
 *   no_chunk_left_behind, asr_queue_full
 * WHAT:  An ordered holding area for closed chunks the ASR queue had no room
 *        for, handed back to it, oldest first, as soon as it does.
 * WHY:   The worker's queue is deliberately tiny (see worker.rs QUEUE_DEPTH), and
 *        a full queue used to DROP the chunk. A dropped chunk is speech that was
 *        recorded, paid for and then silently missing from the transcript — it
 *        read as the model mishearing, and nothing anywhere said otherwise.
 *        Audio chunks are small (a few hundred KB) and the number held is
 *        capped, so waiting costs almost nothing, whereas losing words cannot be
 *        undone. The cap is what keeps "wait" from becoming an unbounded queue
 *        on a machine that has genuinely fallen behind realtime: past it the
 *        OLDEST chunk is evicted and logged, so the loss is at least visible.
 *
 *        Order is the contract. Chunks leave in the order they were pushed, and a
 *        refusal stops the drain, so a newer chunk can never overtake an older
 *        one into the engine.
 * WHERE: Owned by session/actor.rs, which pushes every closed interior chunk and
 *        drains on each decode result; drained into pipeline/worker.rs::offer.
 */

use std::collections::VecDeque;

use crate::ports::engine::TranscribeRequest;
use crate::types::{AudioChunk, SessionId};

/// About two minutes of audio at the longest chunk length. More than that means
/// decode is hopelessly behind and the oldest audio is the least useful.
pub const MAX_BACKLOG: usize = 12;

/**
 * SOURCE OF TRUTH KEYWORDS: Deferred
 * WHAT:  One chunk waiting for a slot in the ASR queue, with everything the
 *        worker needs to decode it.
 * WHERE: Pushed by the actor; handed to AsrWorker::offer.
 */
pub struct Deferred {
    pub chunk: AudioChunk,
    pub request: TranscribeRequest,
    pub session_id: SessionId,
}

#[derive(Default)]
pub struct ChunkBacklog {
    queue: VecDeque<Deferred>,
}

impl ChunkBacklog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Forgets everything held: a cancelled recording's audio must not outlive it.
    pub fn clear(&mut self) {
        self.queue.clear();
    }

    /// Adds a chunk behind the ones already waiting. Evicts the oldest at the cap.
    pub fn push(&mut self, job: Deferred) {
        if self.queue.len() >= MAX_BACKLOG {
            if let Some(lost) = self.queue.pop_front() {
                tracing::error!(
                    start_ms = lost.chunk.start_ms,
                    held = MAX_BACKLOG,
                    "decode is far behind realtime; discarding the oldest waiting chunk"
                );
            }
        }
        self.queue.push_back(job);
    }

    /**
     * WHAT:  Offers waiting chunks to `offer` oldest first until it refuses one,
     *        returning how many it took.
     * WHY:   A refused chunk goes back to the FRONT, so the next drain starts
     *        exactly where this one stopped. It comes back boxed because a chunk
     *        is large and a `Result` the size of its error is passed by value on
     *        every call, including the ones that succeed.
     */
    pub fn drain_into(&mut self, mut offer: impl FnMut(Deferred) -> Result<(), Box<Deferred>>) -> usize {
        let mut taken = 0;
        while let Some(job) = self.queue.pop_front() {
            match offer(job) {
                Ok(()) => taken += 1,
                Err(refused) => {
                    self.queue.push_front(*refused);
                    break;
                }
            }
        }
        taken
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ChunkKind;

    fn job(start_ms: u64) -> Deferred {
        Deferred {
            chunk: AudioChunk {
                samples: vec![0.1; 16],
                start_ms,
                end_ms: start_ms + 1,
                kind: ChunkKind::Interior,
            },
            request: TranscribeRequest::default(),
            session_id: SessionId::new(),
        }
    }

    #[test]
    fn chunks_leave_in_the_order_they_arrived() {
        let mut backlog = ChunkBacklog::new();
        for start in [0, 10, 20] {
            backlog.push(job(start));
        }

        let mut seen = Vec::new();
        let taken = backlog.drain_into(|job| {
            seen.push(job.chunk.start_ms);
            Ok(())
        });

        assert_eq!((taken, seen), (3, vec![0, 10, 20]));
        assert!(backlog.is_empty());
    }

    #[test]
    fn a_refusal_stops_the_drain_and_keeps_the_refused_chunk_at_the_front() {
        let mut backlog = ChunkBacklog::new();
        for start in [0, 10, 20] {
            backlog.push(job(start));
        }

        let mut room = 1;
        let taken = backlog.drain_into(|job| {
            if room == 0 {
                return Err(Box::new(job));
            }
            room -= 1;
            Ok(())
        });
        assert_eq!(taken, 1);
        assert_eq!(backlog.len(), 2);

        // The next drain resumes at 10, not 20: nothing overtook anything.
        let mut seen = Vec::new();
        backlog.drain_into(|job| {
            seen.push(job.chunk.start_ms);
            Ok(())
        });
        assert_eq!(seen, vec![10, 20]);
    }

    #[test]
    fn past_the_cap_the_oldest_chunk_is_the_one_given_up() {
        let mut backlog = ChunkBacklog::new();
        for start in 0..=MAX_BACKLOG as u64 {
            backlog.push(job(start));
        }
        assert_eq!(backlog.len(), MAX_BACKLOG);

        let mut first = None;
        backlog.drain_into(|job| {
            first.get_or_insert(job.chunk.start_ms);
            Ok(())
        });
        assert_eq!(first, Some(1), "chunk 0 was evicted to make room");
    }

    #[test]
    fn clearing_forgets_a_cancelled_recordings_audio() {
        let mut backlog = ChunkBacklog::new();
        backlog.push(job(0));
        backlog.clear();
        assert!(backlog.is_empty());
    }
}
