/*!
 * SOURCE OF TRUTH KEYWORDS: PreviewGate, hold, PreviewOutcome, preview_before_paste,
 *   escape_cancels_preview, withheld
 * WHAT:  The pause between "the text is ready" and "the text is pasted": it puts
 *        the finished text on the caption HUD for a moment and waits — for the
 *        time to run out (paste) or for Escape (withhold).
 * WHY:   A deliberate delay is the one thing in this app that makes delivery
 *        SLOWER, so it is opt-in, bounded (`preview_ms` is clamped to 0.5–5s)
 *        and cancellable at every instant. The wait is a `select!` between a
 *        timer and a one-shot cancel signal, so there is no polling and no state
 *        that can be left armed: `finish` always disarms, and a cancel that
 *        arrives after the timer has won is simply nothing.
 *
 *        THE GATE IS SHARED STATE, and that is the point of it. Escape is
 *        handled by the global-shortcut callback in bootstrap, which has no idea
 *        a delivery is in flight; the delivery worker has no idea Escape was
 *        pressed. The gate is the one object both can reach (it hangs off
 *        AppState and SessionContext), and it answers the only question Escape
 *        needs: "was there a preview to cancel?" — so the key press is consumed
 *        by exactly one meaning, never both this and "cancel the recording".
 *
 *        Escape belongs to a LIVE RECORDING first. If the user has started the
 *        next dictation while a preview is up, Escape cancels that recording
 *        (the actor owns it) and the preview simply runs its course — the
 *        caller checks the session state before offering the key to the gate.
 *
 *        Withholding is not deleting. The text is saved to Dictations either
 *        way — it is only kept out of the document — because a preview that
 *        could destroy words would make people afraid to dictate.
 * WHERE: `hold` is called by session/delivery.rs between enhancement and
 *        injection; `PreviewGate::cancel` by bootstrap's Escape handler.
 */

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::sync::oneshot;

use crate::ipc::context::SessionContext;
use crate::types::{Caption, SessionState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewOutcome {
    /// The time ran out; paste it.
    Confirmed,
    /// Escape was pressed; keep it out of the document.
    Withheld,
}

/// The one cancel signal for the preview that is currently up, if any.
#[derive(Clone, Default)]
pub struct PreviewGate {
    pending: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl PreviewGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// Arms the gate and returns the receiver that resolves when it is cancelled.
    fn arm(&self) -> oneshot::Receiver<()> {
        let (sender, receiver) = oneshot::channel();
        *self.pending.lock() = Some(sender);
        receiver
    }

    /// Disarms it. Always called, whichever way the wait ended.
    fn disarm(&self) {
        self.pending.lock().take();
    }

    /// Whether a preview is waiting to be cancelled.
    pub fn is_active(&self) -> bool {
        self.pending.lock().is_some()
    }

    /// Cancels the preview that is up. True when there was one — i.e. when the
    /// caller's Escape press has been used.
    pub fn cancel(&self) -> bool {
        match self.pending.lock().take() {
            Some(sender) => sender.send(()).is_ok(),
            None => false,
        }
    }
}

/**
 * WHAT:  Shows `text` on the caption HUD and waits `hold_ms` or until Escape.
 * WHY:   Hides the HUD afterwards ONLY when no recording is live: the same window
 *        shows live captions, and a preview ending beneath a new dictation must
 *        not blank the captions of the one that has started.
 * WHERE: session/delivery.rs.
 */
pub async fn hold(ctx: &SessionContext, text: &str, hold_ms: u64) -> PreviewOutcome {
    let cancelled = ctx.preview.arm();
    ctx.ports.events.caption_changed(Some(&Caption::Preview { text: text.to_string(), hold_ms }));

    let outcome = tokio::select! {
        () = tokio::time::sleep(Duration::from_millis(hold_ms)) => PreviewOutcome::Confirmed,
        // A dropped sender (the gate was re-armed) is not a cancel.
        result = cancelled => if result.is_ok() { PreviewOutcome::Withheld } else { PreviewOutcome::Confirmed },
    };

    ctx.preview.disarm();
    if matches!(ctx.current_state(), SessionState::Idle) {
        ctx.ports.events.caption_changed(None);
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelling_with_nothing_to_cancel_says_so_and_leaves_escape_free() {
        let gate = PreviewGate::new();
        assert!(!gate.is_active());
        assert!(!gate.cancel(), "no preview was up, so the key press was not used");
    }

    #[tokio::test]
    async fn a_cancel_resolves_the_armed_wait_exactly_once() {
        let gate = PreviewGate::new();
        let receiver = gate.arm();
        assert!(gate.is_active());

        assert!(gate.cancel(), "the first cancel is used");
        assert!(!gate.cancel(), "a second has nothing to cancel");
        assert!(receiver.await.is_ok(), "the waiter was told");
        assert!(!gate.is_active());
    }

    #[tokio::test]
    async fn disarming_leaves_a_late_cancel_with_nothing_to_do() {
        let gate = PreviewGate::new();
        let _receiver = gate.arm();
        gate.disarm();
        assert!(!gate.cancel(), "the timer won; a late Escape is not ours");
    }
}
