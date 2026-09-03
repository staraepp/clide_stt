//! Permission commands.
//!
//! Requests are only ever called from onboarding, and every one of them
//! re-reads the real system state afterwards instead of trusting the prompt.

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

#[tauri::command]
pub fn open_accessibility_settings() {
    permissions::open_accessibility_settings();
}

#[tauri::command]
pub fn open_microphone_settings() {
    permissions::open_microphone_settings();
}
