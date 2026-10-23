/*!
 * SOURCE OF TRUTH KEYWORDS: SpeakerDucker, DuckGuard, NullSpeakerDucker, quiet_speakers,
 *   meeting_bleed, duck, recover
 * WHAT:  The trait for silencing the computer's own speakers for the length of
 *        a recording, and the guard that gives them back.
 * WHY:   In a call the other people's voices come out of the speakers, and a
 *        microphone a few centimetres away records them as faithfully as it
 *        records you — so a dictation started mid-meeting transcribes the
 *        meeting. A single microphone cannot tell voices apart, so the fix is
 *        not to let the other voices reach it: the speakers are quiet while
 *        Vocs is listening. Restoring is tied to the guard being dropped rather
 *        than to a matching call, so no path out of a recording (cancel,
 *        failure, a lost device) can leave the Mac muted.
 *
 *        `recover` exists because a guard cannot run when the process dies. An
 *        adapter that changes system state records it durably and undoes it
 *        here at the next launch.
 * WHERE: Implemented by adapters/macos/speaker_duck.rs; driven by the session
 *        actor around start_capture / stop_capture.
 */

/// Held for as long as the speakers should stay quiet; dropping it restores
/// them. Carries no methods — the drop IS the interface.
pub trait DuckGuard: Send {}

pub trait SpeakerDucker: Send + Sync {
    /// Quiets the speakers if they are the output the microphone could hear.
    /// `None` means there was nothing to do (headphones, already muted, no
    /// control available) and is not a failure.
    fn duck(&self) -> Option<Box<dyn DuckGuard>>;

    /// Undoes a duck an earlier run never got to undo.
    fn recover(&self);
}

/**
 * SOURCE OF TRUTH KEYWORDS: NullSpeakerDucker
 * WHAT:  A ducker that never touches the system.
 * WHY:   For tests and for platforms with no adapter, so the actor never needs
 *        an Option.
 */
pub struct NullSpeakerDucker;

impl SpeakerDucker for NullSpeakerDucker {
    fn duck(&self) -> Option<Box<dyn DuckGuard>> {
        None
    }

    fn recover(&self) {}
}
