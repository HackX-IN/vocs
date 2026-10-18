/*!
 * SOURCE OF TRUTH KEYWORDS: save_text_file, SaveTextFileInput, save_dialog,
 *   suggested_name, open_text_file, OpenTextFileInput, MAX_IMPORT_BYTES
 * WHAT:  Writes text to a file the user picks, through a native save dialog.
 * WHY:   A webview `Blob` + download anchor cannot be trusted here. Our CSP is
 *        restrictive, the download is sandboxed, and — critically — when it
 *        fails it fails SILENTLY: the click appears to work and no file
 *        appears. For history export that is the worst possible failure, since
 *        the whole feature exists so someone's transcripts are not locked
 *        inside this app. A person who believes they have a backup and does
 *        not is worse off than one who knows they have none.
 *
 *        So the write goes through Rust, where a failure is an AppError the UI
 *        can actually show, and a success is a real path on disk.
 *
 *        `open_text_file` is the same argument mirrored: a webview file input
 *        hands the page a File it can read, but only after the page has been
 *        granted a path-less handle, and the same restrictive CSP applies. Rust
 *        reads the file so a failure is an AppError and the size is bounded
 *        BEFORE the bytes are pulled into memory.
 * WHERE: Called by the History export action and the Settings backup panel.
 */

use std::io::{Read, Write};

use tauri::State;
use tauri_plugin_dialog::DialogExt;

use crate::error::{AppError, ErrorCode};
use crate::ipc::context::AppState;
use crate::ipc::factory::{execute, CommandSpec, Validate};
use crate::registry::CapabilityKey;

/// Generous, but bounded — this is a transcript archive, not a disk image.
const MAX_EXPORT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct SaveTextFileInput {
    pub contents: String,
    /// Offered in the dialog. The user may change it.
    pub suggested_name: String,
}

impl Validate for SaveTextFileInput {
    fn validate(&self) -> Result<(), String> {
        if self.contents.is_empty() {
            return Err("There is nothing to save.".into());
        }
        if self.contents.len() > MAX_EXPORT_BYTES {
            return Err("That export is too large to save in one file.".into());
        }
        if self.suggested_name.trim().is_empty() {
            return Err("A file name is required.".into());
        }
        // A name is not a path. Refusing separators here stops a suggested name
        // from steering the dialog somewhere the user did not choose.
        if self.suggested_name.contains('/') || self.suggested_name.contains("..") {
            return Err("That file name is not allowed.".into());
        }
        Ok(())
    }
}

const SAVE: CommandSpec =
    CommandSpec::new("save_text_file", CapabilityKey::History).exclusive();

/**
 * WHAT:  Shows a save dialog and writes the text to whatever the user picks.
 * WHY:   Returns the chosen path, or None when the user cancelled. Cancelling
 *        is NOT an error — treating it as one would make the UI apologise for
 *        something the user deliberately did.
 * WHERE: The History export flow.
 */
#[tauri::command]
#[specta::specta]
pub async fn save_text_file(
    state: State<'_, AppState>,
    input: SaveTextFileInput,
) -> Result<Option<String>, AppError> {
    execute(&state, SAVE, input, |ctx, input| async move {
        let (tx, rx) = tokio::sync::oneshot::channel();

        ctx.state
            .app
            .dialog()
            .file()
            .set_file_name(&input.suggested_name)
            .save_file(move |path| {
                let _ = tx.send(path);
            });

        // The dialog is modal to the user but not to us; awaiting the callback
        // keeps this command from returning before there is an answer.
        let Ok(Some(path)) = rx.await else {
            return Ok(None);
        };

        let path = path.into_path().map_err(|err| {
            AppError::new(ErrorCode::Io, "That location cannot be written to.").with_detail(err)
        })?;

        // Written through a temp file and renamed, so an interrupted write
        // cannot leave a truncated archive that looks complete.
        let temp = path.with_extension("part");
        {
            let mut file = std::fs::File::create(&temp)?;
            file.write_all(input.contents.as_bytes())?;
            file.sync_all()?;
        }
        std::fs::rename(&temp, &path)?;

        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
}

/// A restored file is read whole into memory and parsed as JSON, so the ceiling
/// is set by what is sane to hold, not by what the disk can supply.
const MAX_IMPORT_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, serde::Deserialize, specta::Type)]
pub struct OpenTextFileInput {
    /// Shown in the dialog's file-type dropdown, e.g. "Vocs backup".
    pub filter_name: String,
    /// Without the dot, e.g. ["json"].
    pub extensions: Vec<String>,
}

impl Validate for OpenTextFileInput {
    fn validate(&self) -> Result<(), String> {
        if self.filter_name.trim().is_empty() || self.extensions.is_empty() {
            return Err("A file type is required.".into());
        }
        // An extension is letters and digits, never a glob or a path.
        if self
            .extensions
            .iter()
            .any(|ext| ext.is_empty() || !ext.chars().all(|c| c.is_ascii_alphanumeric()))
        {
            return Err("That file type is not allowed.".into());
        }
        Ok(())
    }
}

const OPEN: CommandSpec = CommandSpec::new("open_text_file", CapabilityKey::Settings).exclusive();

/**
 * WHAT:  Shows an open dialog and returns the picked file's text, or None when
 *        the user cancelled.
 * WHY:   Cancelling is not an error, for the same reason as the save dialog.
 *        The file must be UTF-8 text: a binary file picked by mistake gets a
 *        sentence saying so, not a stack of replacement characters downstream.
 * WHERE: The Settings backup panel's Import action.
 */
#[tauri::command]
#[specta::specta]
pub async fn open_text_file(
    state: State<'_, AppState>,
    input: OpenTextFileInput,
) -> Result<Option<String>, AppError> {
    execute(&state, OPEN, input, |ctx, input| async move {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let extensions: Vec<&str> = input.extensions.iter().map(String::as_str).collect();

        ctx.state
            .app
            .dialog()
            .file()
            .add_filter(&input.filter_name, &extensions)
            .pick_file(move |path| {
                let _ = tx.send(path);
            });

        let Ok(Some(path)) = rx.await else {
            return Ok(None);
        };
        let path = path.into_path().map_err(|err| {
            AppError::new(ErrorCode::Io, "That file cannot be read.").with_detail(err)
        })?;

        let file = std::fs::File::open(&path)?;
        // Check the size on the handle we will read from, not on a path that
        // could be swapped between the check and the read.
        if file.metadata()?.len() > MAX_IMPORT_BYTES {
            return Err(AppError::new(ErrorCode::Io, "That file is too large to import."));
        }
        let mut text = String::new();
        // `take` bounds the read even if the file grows after the check.
        file.take(MAX_IMPORT_BYTES + 1).read_to_string(&mut text).map_err(|err| {
            AppError::new(ErrorCode::Io, "That file is not readable text.").with_detail(err)
        })?;
        if text.len() as u64 > MAX_IMPORT_BYTES {
            return Err(AppError::new(ErrorCode::Io, "That file is too large to import."));
        }
        Ok(Some(text))
    })
    .await
}
