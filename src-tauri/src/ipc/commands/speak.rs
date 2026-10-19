/*!
 * SOURCE OF TRUTH KEYWORDS: list_voices, list_output_devices, get_speak_status,
 *   set_safe_mode, request_personal_voice, test_voice, start_speaking, voice_mode_commands
 * WHAT:  What the Voice settings page needs: the voices and devices to pick from,
 *        Personal Voice permission, a test button, and the meeting-safe-mode
 *        switch with its current state.
 * WHY:   Listing voices and devices is not free (Core Audio and AVFoundation
 *        round trips), so the lists are fetched once when the page opens and not
 *        polled; the live part — speaking, safe mode — is pushed as
 *        SpeakStatusChanged. The test and the switches run on the blocking pool
 *        because they open audio streams, and the factory's reentrancy guard keeps
 *        two of them from fighting over one device.
 * WHERE: Consumed by Settings → Voice and the menu bar.
 */

use tauri::State;

use crate::error::AppError;
use crate::ipc::context::AppState;
use crate::ipc::factory::{execute, CommandSpec, Validate};
use crate::registry::CapabilityKey;
use crate::session::SessionSettings;
use crate::types::{AudioOutputDevice, PersonalVoiceStatus, SpeakStatus, VoiceInfo};

const VOICES: CommandSpec = CommandSpec::new("list_voices", CapabilityKey::Speak);

#[tauri::command]
#[specta::specta]
pub async fn list_voices(state: State<'_, AppState>) -> Result<Vec<VoiceInfo>, AppError> {
    execute(&state, VOICES, (), |ctx, ()| async move {
        let speak = ctx.speak().clone();
        tokio::task::spawn_blocking(move || speak.speech().voices()).await.map_err(AppError::internal)
    })
    .await
}

const DEVICES: CommandSpec = CommandSpec::new("list_output_devices", CapabilityKey::Speak);

#[tauri::command]
#[specta::specta]
pub async fn list_output_devices(state: State<'_, AppState>) -> Result<Vec<AudioOutputDevice>, AppError> {
    execute(&state, DEVICES, (), |ctx, ()| async move {
        let speak = ctx.speak().clone();
        tokio::task::spawn_blocking(move || speak.speech().output_devices()).await.map_err(AppError::internal)
    })
    .await
}

const STATUS: CommandSpec = CommandSpec::new("get_speak_status", CapabilityKey::Speak);

#[tauri::command]
#[specta::specta]
pub async fn get_speak_status(state: State<'_, AppState>) -> Result<SpeakStatus, AppError> {
    execute(&state, STATUS, (), |ctx, ()| async move { Ok(ctx.speak().status()) }).await
}

#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct SetSafeModeInput {
    pub on: bool,
}

impl Validate for SetSafeModeInput {
    fn validate(&self) -> Result<(), String> {
        Ok(())
    }
}

const SAFE_MODE: CommandSpec = CommandSpec::new("set_safe_mode", CapabilityKey::Speak).exclusive();

#[tauri::command]
#[specta::specta]
pub async fn set_safe_mode(state: State<'_, AppState>, input: SetSafeModeInput) -> Result<SpeakStatus, AppError> {
    execute(&state, SAFE_MODE, input, |ctx, input| async move {
        let speak = ctx.speak().clone();
        let settings = SessionSettings::load(ctx.db());
        tokio::task::spawn_blocking(move || {
            speak.set_safe_mode(&settings, input.on, None)?;
            Ok(speak.status())
        })
        .await
        .map_err(AppError::internal)?
    })
    .await
}

const PERSONAL_VOICE: CommandSpec = CommandSpec::new("request_personal_voice", CapabilityKey::Speak).exclusive();

/// Shows the system's Personal Voice prompt and returns the answer.
#[tauri::command]
#[specta::specta]
pub async fn request_personal_voice(state: State<'_, AppState>) -> Result<PersonalVoiceStatus, AppError> {
    execute(&state, PERSONAL_VOICE, (), |ctx, ()| async move {
        let (tx, rx) = tokio::sync::oneshot::channel();
        ctx.speak().speech().request_personal_voice(Box::new(move |status| {
            let _ = tx.send(status);
        }));
        rx.await.map_err(AppError::internal)
    })
    .await
}

const PERSONAL_STATUS: CommandSpec = CommandSpec::new("get_personal_voice_status", CapabilityKey::Speak);

#[tauri::command]
#[specta::specta]
pub async fn get_personal_voice_status(state: State<'_, AppState>) -> Result<PersonalVoiceStatus, AppError> {
    execute(&state, PERSONAL_STATUS, (), |ctx, ()| async move {
        Ok(ctx.speak().speech().personal_voice_status())
    })
    .await
}

const TEST: CommandSpec = CommandSpec::new("test_voice", CapabilityKey::Speak).exclusive();

/// Says a test sentence with the chosen voice and device. Returns how long the
/// first sound took, the number a meeting feels as dead air.
#[tauri::command]
#[specta::specta]
pub async fn test_voice(state: State<'_, AppState>) -> Result<u32, AppError> {
    execute(&state, TEST, (), |ctx, ()| async move {
        let speak = ctx.speak().clone();
        let settings = SessionSettings::load(ctx.db());
        tokio::task::spawn_blocking(move || speak.test_voice(&settings).map(|report| report.first_audio_ms as u32))
            .await
            .map_err(AppError::internal)?
    })
    .await
}
