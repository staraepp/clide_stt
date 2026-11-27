//! Windows desktop apps use the system microphone privacy policy; capture reports device errors.
use serde::Serialize;
use std::os::windows::process::CommandExt;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionStatus {
    /// Never asked. Prompting will show the system dialog.
    NotDetermined,
    Granted,
    /// Refused. Only System Settings can change this now.
    Denied,
    /// Blocked by policy (managed device, Screen Time).
    Restricted,
}

impl PermissionStatus {
    pub fn is_granted(self) -> bool {
        matches!(self, Self::Granted)
    }
}

/// Everything Clide needs, in one read.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionSnapshot {
    pub microphone: PermissionStatus,
    pub accessibility: PermissionStatus,
    /// Only needed by Apple Speech. Every other engine ignores it, so Clide
    /// never prompts for it unless that provider is selected.
    pub speech_recognition: PermissionStatus,
}

impl PermissionSnapshot {
    /// Dictation can capture audio but may not be able to type it anywhere.
    pub fn can_capture(&self) -> bool {
        self.microphone.is_granted()
    }

    /// The full pipeline is available.
    pub fn can_insert(&self) -> bool {
        self.accessibility.is_granted()
    }
}

pub fn is_ad_hoc() -> bool {
    false
}
fn microphone_status() -> PermissionStatus {
    for root in ["HKCU", "HKLM"] {
        let path=format!("{root}\\Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone");
        if let Ok(out) = std::process::Command::new("reg")
            .args(["query", &path, "/v", "Value"])
            .creation_flags(0x08000000)
            .output()
        {
            if String::from_utf8_lossy(&out.stdout)
                .split_whitespace()
                .any(|s| s == "Deny")
            {
                return PermissionStatus::Denied;
            }
        }
    }
    PermissionStatus::Granted
}
pub fn snapshot() -> PermissionSnapshot {
    PermissionSnapshot {
        microphone: microphone_status(),
        accessibility: PermissionStatus::Granted,
        speech_recognition: PermissionStatus::Restricted,
    }
}
pub fn accessibility_status() -> PermissionStatus {
    PermissionStatus::Granted
}
pub fn request_microphone_access() -> PermissionStatus {
    let status = microphone_status();
    if !status.is_granted() {
        open_microphone_settings();
    }
    status
}
pub fn request_speech_access() -> PermissionStatus {
    PermissionStatus::Restricted
}
pub fn request_accessibility_access() -> PermissionStatus {
    PermissionStatus::Granted
}
pub fn repair_accessibility_access(_: &str) -> Result<PermissionStatus, String> {
    Ok(PermissionStatus::Granted)
}
pub fn open_accessibility_settings() {}
pub fn open_microphone_settings() {
    let _ = std::process::Command::new("explorer.exe")
        .arg("ms-settings:privacy-microphone")
        .spawn();
}
