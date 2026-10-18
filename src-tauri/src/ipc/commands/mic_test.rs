/*!
 * SOURCE OF TRUTH KEYWORDS: start_mic_test, stop_mic_test, MicTestChanged,
 *   mic_check, input_level_meter
 * WHAT:  Starts and stops the Settings microphone level check on the input
 *        device dictation would use.
 * WHY:   The device comes from the stored setting rather than from the caller,
 *        so the meter can only ever show what a real recording will hear — a
 *        meter pointed at a different device than dictation is worse than none.
 *        Starting is refused while a recording is in progress: the recorder owns
 *        the microphone then, and a second stream would only add load and a
 *        meter that disagrees with the pill. Stopping needs no grant and never
 *        fails, because the one time it matters is when something has already
 *        gone wrong.
 * WHERE: Called by the MicTestPanel in Settings → Recording; levels come back
 *        on the mic-test-changed event, never as a return value.
 */

use tauri::State;

use crate::error::{AppError, ErrorCode};
use crate::ipc::context::AppState;
use crate::ipc::factory::{execute, CommandSpec};
use crate::registry::CapabilityKey;
use crate::session::SessionSettings;
use crate::types::SessionState;

// Dictation, so the factory's preflight demands the microphone grant and the
// failure is the same actionable error a real recording would give.
const START: CommandSpec = CommandSpec::new("start_mic_test", CapabilityKey::Dictation);
const STOP: CommandSpec = CommandSpec::new("stop_mic_test", CapabilityKey::Dictation).reports();

#[tauri::command]
#[specta::specta]
pub async fn start_mic_test(state: State<'_, AppState>) -> Result<(), AppError> {
    execute(&state, START, (), |ctx, ()| async move {
        if !matches!(ctx.session_state(), SessionState::Idle) {
            return Err(AppError::new(
                ErrorCode::SessionAlreadyActive,
                "Finish the current recording before checking the microphone.",
            )
            .recoverable());
        }
        let device = SessionSettings::load(ctx.db()).input_device;
        ctx.mic_monitor().start(device)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn stop_mic_test(state: State<'_, AppState>) -> Result<(), AppError> {
    execute(&state, STOP, (), |ctx, ()| async move {
        ctx.mic_monitor().stop();
        Ok(())
    })
    .await
}
