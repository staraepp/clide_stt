//! Commands for the local model manager.
//!
//! Browsing, downloading, and removing weights. Transcribing with them goes
//! through the normal provider path — a local engine is not a special case.

use tauri::{AppHandle, Emitter, Manager};

use crate::models::{catalog, download, ModelStatus};
use crate::state::AppState;

/// Every installable model, annotated with what is on this machine.
#[tauri::command]
pub fn list_models(app: AppHandle) -> Vec<ModelStatus> {
    app.state::<AppState>().models.catalog_status()
}

/// Download a model's weights.
///
/// Returns as soon as the transfer is under way; progress arrives as
/// `model:progress` events and settles on `model:complete` or `model:failed`.
/// Blocking here would freeze the UI for the length of a multi-gigabyte
/// download.
#[tauri::command]
pub fn download_model(app: AppHandle, model_id: String) -> Result<(), String> {
    let entry = catalog::find(&model_id).ok_or_else(|| format!("no model called {model_id}"))?;

    {
        let state = app.state::<AppState>();
        if state.models.is_installed(&entry) {
            return Ok(());
        }
    }

    tauri::async_runtime::spawn(async move {
        let (models, http) = {
            let state = app.state::<AppState>();
            (state.models.clone(), state.http.clone())
        };

        if let Err(error) = download::download(&app, &models, &entry, &http).await {
            tracing::warn!(model = %entry.id, %error, "model download failed");
            let _ = app.emit(
                download::EVENT_FAILED,
                download::Failed {
                    model_id: entry.id.clone(),
                    message: error.to_string(),
                },
            );
        } else {
            tracing::info!(model = %entry.id, "model installed");
        }
    });

    Ok(())
}

/// Delete a model's weights, freeing the disk space.
#[tauri::command]
pub fn remove_model(app: AppHandle, model_id: String) -> Result<(), String> {
    let entry = catalog::find(&model_id).ok_or_else(|| format!("no model called {model_id}"))?;

    app.state::<AppState>()
        .models
        .remove(&entry)
        .map_err(|error| error.to_string())
}
