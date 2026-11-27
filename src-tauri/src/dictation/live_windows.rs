//! Apple live typing is not exposed on Windows; local capture recognition runs instead.
use tauri::AppHandle;
pub type LiveSlot = std::sync::Mutex<Option<()>>;
pub struct LiveOutcome {
    pub text: String,
}
pub fn is_active(_: &AppHandle) -> bool {
    false
}
pub fn begin(_: &AppHandle, _: Option<String>) -> bool {
    false
}
pub fn cancel(_: &AppHandle) {}
pub fn finish(_: &AppHandle) -> Option<LiveOutcome> {
    None
}
