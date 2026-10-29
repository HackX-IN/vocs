/*!
 * SOURCE OF TRUTH KEYWORDS: UpdateCache, FRESH_FOR, UPDATE_HANDLE_FOR, check_gate,
 *   cached_update_check, one_request_per_burst
 * WHAT:  Remembers the answer to the last update check, and the downloadable
 *        `Update` that came with it, so asking again within a few minutes costs
 *        nothing — and so two callers asking at once make one request between them.
 * WHY:   "Check for updates takes a long time" had three causes, and this is the
 *        second and third. The dashboard bar and the Settings card each asked the
 *        server on every mount, and installing asked a THIRD time before it would
 *        start: three round trips to GitHub (two redirects each) for one decision.
 *        An answer that is a few minutes old is as good as a new one for "is there
 *        something newer", so a plain look-up reuses it and only the button the
 *        person pressed (or the daily timer) goes back to the network.
 *
 *        The `gate` serialises requests rather than rejecting them: a second
 *        caller waits for the first and then finds its answer in the cache, which
 *        is what turns a burst of mounts into one request instead of an error.
 *
 *        The `Update` is kept so installing does not have to look again — an
 *        `Update` cannot cross IPC, but it can sit here, in Rust, beside the
 *        answer it belongs to. It is kept for a shorter time than it is trusted
 *        for display, and the download re-verifies the signature regardless, so a
 *        release pulled in between fails safely instead of installing.
 * WHERE: Held by AppState; read and written by ipc/commands/updates.rs.
 */

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tauri_plugin_updater::Update;

use super::commands::updates::UpdateCheck;

/// How long a non-forced look-up may reuse the last answer.
const FRESH_FOR: Duration = Duration::from_secs(10 * 60);

/// How long the downloadable `Update` is trusted for an install click.
const UPDATE_HANDLE_FOR: Duration = Duration::from_secs(60 * 60);

struct Cached {
    at: Instant,
    check: UpdateCheck,
    update: Option<Update>,
}

#[derive(Clone, Default)]
pub struct UpdateCache {
    last: Arc<Mutex<Option<Cached>>>,
    gate: Arc<tokio::sync::Mutex<()>>,
}

impl UpdateCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// The last answer, if it is recent enough to reuse.
    pub fn fresh(&self) -> Option<UpdateCheck> {
        let last = self.last.lock();
        last.as_ref().filter(|cached| cached.at.elapsed() < FRESH_FOR).map(|cached| cached.check.clone())
    }

    /// Records an answer, and the update it offers when it offers one.
    pub fn store(&self, check: UpdateCheck, update: Option<Update>) {
        *self.last.lock() = Some(Cached { at: Instant::now(), check, update });
    }

    /// The downloadable update from the last check, if there is one and it is
    /// still young enough to install without looking again.
    pub fn available_update(&self) -> Option<Update> {
        let last = self.last.lock();
        last.as_ref()
            .filter(|cached| cached.at.elapsed() < UPDATE_HANDLE_FOR)
            .and_then(|cached| cached.update.clone())
    }

    /// Forgets everything: updates were switched off, or one was installed.
    pub fn forget(&self) {
        *self.last.lock() = None;
    }

    /// Waits for any check already on the wire. Held for the length of a request.
    pub async fn gate(&self) -> tokio::sync::OwnedMutexGuard<()> {
        Arc::clone(&self.gate).lock_owned().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn current() -> UpdateCheck {
        UpdateCheck::UpToDate { current_version: "1.0.0".into() }
    }

    #[test]
    fn a_new_cache_has_nothing_to_reuse() {
        let cache = UpdateCache::new();
        assert!(cache.fresh().is_none());
        assert!(cache.available_update().is_none());
    }

    #[test]
    fn a_stored_answer_is_reused_until_it_is_forgotten() {
        let cache = UpdateCache::new();
        cache.store(current(), None);
        assert!(matches!(cache.fresh(), Some(UpdateCheck::UpToDate { .. })));
        assert!(cache.available_update().is_none(), "an up-to-date answer offers nothing to install");

        cache.forget();
        assert!(cache.fresh().is_none());
    }

    #[test]
    fn an_old_answer_is_not_reused() {
        let cache = UpdateCache::new();
        *cache.last.lock() = Some(Cached {
            at: Instant::now().checked_sub(FRESH_FOR + Duration::from_secs(1)).expect("clock is past the window"),
            check: current(),
            update: None,
        });
        assert!(cache.fresh().is_none());
    }

    #[tokio::test]
    async fn the_gate_makes_a_second_caller_wait_for_the_first() {
        let cache = UpdateCache::new();
        let first = cache.gate().await;

        let waiting = tokio::spawn({
            let cache = cache.clone();
            async move {
                let _second = cache.gate().await;
            }
        });
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(!waiting.is_finished(), "the second caller got through while the first held the gate");

        drop(first);
        waiting.await.expect("the second caller runs once the first is done");
    }
}
