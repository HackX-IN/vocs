/*!
 * SOURCE OF TRUTH KEYWORDS: list_models, download_model, delete_model,
 *   get_model_status, ModelIdInput
 * WHAT:  The model manager: what is available, what is on disk, and downloading
 *        or removing it.
 * WHY:   Download is Exclusive so two concurrent calls cannot write the same
 *        partial file — resumable downloads share a `.part` path, and two
 *        writers to it would produce a file that hashes to nothing and looks
 *        like a corrupt server.
 * WHERE: Consumed by onboarding and the model manager in Settings.
 */

use tauri::State;

use crate::error::AppError;
use crate::ipc::context::{AppState, CommandContext};
use crate::ipc::factory::{execute, CommandSpec, Validate};
use crate::ports::models::ModelStatus;
use crate::registry::CapabilityKey;
use crate::session::grammar_runtime::{adopt_grammar_model, release_grammar_model};
use crate::types::{ModelId, ModelKind, ModelState};

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ModelReport {
    pub descriptor: crate::types::ModelDescriptor,
    pub state: ModelState,
    /// Absolute path, present only once the file is hash-verified.
    pub path: Option<String>,
}

impl From<ModelStatus> for ModelReport {
    fn from(status: ModelStatus) -> Self {
        Self {
            descriptor: status.descriptor,
            state: status.state,
            path: status.path.map(|p| p.to_string_lossy().into_owned()),
        }
    }
}

/// Whether this id names the grammar model, which needs registering with its
/// runtime in addition to being a file on disk.
async fn is_grammar_model(ctx: &CommandContext, id: &ModelId) -> bool {
    ctx.ports()
        .models
        .status(id)
        .await
        .is_ok_and(|status| status.descriptor.kind == ModelKind::Grammar)
}

const LIST: CommandSpec = CommandSpec::new("list_models", CapabilityKey::Models);

#[tauri::command]
#[specta::specta]
pub async fn list_models(state: State<'_, AppState>) -> Result<Vec<ModelReport>, AppError> {
    execute(&state, LIST, (), |ctx, ()| async move {
        Ok(ctx
            .ports()
            .models
            .list()
            .await?
            .into_iter()
            .map(ModelReport::from)
            .collect())
    })
    .await
}

#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct ModelIdInput {
    pub model_id: ModelId,
}

impl Validate for ModelIdInput {
    fn validate(&self) -> Result<(), String> {
        if self.model_id.as_str().trim().is_empty() {
            return Err("A model is required.".into());
        }
        Ok(())
    }
}

const STATUS: CommandSpec = CommandSpec::new("get_model_status", CapabilityKey::Models);

#[tauri::command]
#[specta::specta]
pub async fn get_model_status(
    state: State<'_, AppState>,
    input: ModelIdInput,
) -> Result<ModelReport, AppError> {
    execute(&state, STATUS, input, |ctx, input| async move {
        Ok(ctx.ports().models.status(&input.model_id).await?.into())
    })
    .await
}

/// Downloads if absent, verifies by hash, and returns the verified path.
/// Progress arrives on the ModelDownloadProgress event, not from this call.
const DOWNLOAD: CommandSpec =
    CommandSpec::new("download_model", CapabilityKey::Models).exclusive();

#[tauri::command]
#[specta::specta]
pub async fn download_model(
    state: State<'_, AppState>,
    input: ModelIdInput,
) -> Result<String, AppError> {
    execute(&state, DOWNLOAD, input, |ctx, input| async move {
        let path = ctx.ports().models.ensure(&input.model_id).await?;
        // A grammar model is registered the moment it is verified, so the
        // setting works without a restart.
        if is_grammar_model(&ctx, &input.model_id).await {
            adopt_grammar_model(ctx.grammar(), path.clone());
        }
        Ok(path.to_string_lossy().into_owned())
    })
    .await
}

const DELETE: CommandSpec = CommandSpec::new("delete_model", CapabilityKey::Models).exclusive();

#[tauri::command]
#[specta::specta]
pub async fn delete_model(
    state: State<'_, AppState>,
    input: ModelIdInput,
) -> Result<(), AppError> {
    execute(&state, DELETE, input, |ctx, input| async move {
        let grammar = is_grammar_model(&ctx, &input.model_id).await;
        ctx.ports().models.delete(&input.model_id).await?;
        if grammar {
            release_grammar_model(ctx.grammar());
        }
        Ok(())
    })
    .await
}
