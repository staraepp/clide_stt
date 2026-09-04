//! Permission commands.
//!
//! Requests are only ever called from onboarding, and every one of them
//! re-reads the real system state afterwards instead of trusting the prompt.

use tauri::AppHandle;

use crate::permissions::{self, PermissionSnapshot, PermissionStatus};

#[tauri::command]
pub fn get_permissions() -> PermissionSnapshot {
    permissions::snapshot()
}

#[tauri::command]
pub async fn request_microphone_permission() -> PermissionStatus {
    // Blocking: the prompt is modal and the completion handler arrives later.
    tauri::async_runtime::spawn_blocking(permissions::request_microphone_access)
        .await
        .unwrap_or(PermissionStatus::NotDetermined)
}

#[tauri::command]
pub fn request_accessibility_permission() -> PermissionStatus {
    permissions::request_accessibility_access()
}

/// Repair the specific case where System Settings shows Clide enabled but the
/// grant still belongs to an older code signature.
#[tauri::command]
pub fn repair_accessibility_permission(app: AppHandle) -> Result<PermissionStatus, String> {
    permissions::repair_accessibility_access(&app.config().identifier)
}

#[tauri::command]
pub fn open_accessibility_settings() {
    permissions::open_accessibility_settings();
}

#[tauri::command]
pub fn open_microphone_settings() {
    permissions::open_microphone_settings();
}

/// Ask macOS for speech-recognition access.
///
/// Separate from the microphone: macOS treats handing audio to the recogniser
/// as its own consent, even on-device. Only Apple Speech needs it.
#[tauri::command]
pub async fn request_speech_permission() -> Result<crate::permissions::PermissionStatus, String> {
    tauri::async_runtime::spawn_blocking(crate::permissions::request_speech_access)
        .await
        .map_err(|_| "The permission request did not complete.".to_string())
}
