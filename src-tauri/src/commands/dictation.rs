//! Dictation commands. Thin wrappers: the pipeline owns the behaviour.

use tauri::{AppHandle, Manager};

use crate::dictation::machine::DictationState;
use crate::dictation::pipeline;
use crate::state::AppState;

#[tauri::command]
pub async fn start_dictation(app: AppHandle) {
    pipeline::start(&app).await;
}

#[tauri::command]
pub async fn stop_dictation(app: AppHandle) {
    pipeline::stop(&app).await;
}

#[tauri::command]
pub async fn cancel_dictation(app: AppHandle) {
    pipeline::cancel(&app).await;
}

/// Re-send the still-pending audio to the provider after a failure.
#[tauri::command]
pub async fn retry_dictation(app: AppHandle) {
    pipeline::retry(&app).await;
}

/// Acknowledge a finished or failed transaction and clear the HUD.
#[tauri::command]
pub fn dismiss_dictation(app: AppHandle) {
    pipeline::dismiss(&app);
}

/// The authoritative state, for a window that just opened and missed the
/// events it would otherwise have been driven by.
#[tauri::command]
pub fn get_dictation_state(app: AppHandle) -> DictationState {
    app.state::<AppState>().session.state()
}
