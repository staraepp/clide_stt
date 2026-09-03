//! Microphone, Accessibility, and speech-recognition permission state.
//!
//! Clide never asks for a permission at launch. Every prompt here is triggered
//! by the onboarding step that explains why it is needed, and every request is
//! followed by a re-read of the real system state rather than an assumption
//! that the user said yes.

mod microphone;
mod signing;
mod speech;

use serde::Serialize;

pub use microphone::request_microphone_access;
pub use signing::is_ad_hoc;
pub use speech::request as request_speech_access;

use crate::insertion::ax;

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

pub fn snapshot() -> PermissionSnapshot {
    PermissionSnapshot {
        microphone: microphone::status(),
        accessibility: accessibility_status(),
        speech_recognition: speech::status(),
    }
}

/// Accessibility exposes only "trusted or not"; macOS does not tell an app
/// whether the user actively denied it or has simply never been asked, so an
/// untrusted process reports `NotDetermined` and the UI offers System Settings.
pub fn accessibility_status() -> PermissionStatus {
    if ax::is_process_trusted() {
        PermissionStatus::Granted
    } else {
        PermissionStatus::NotDetermined
    }
}

/// Show the system's Accessibility prompt.
///
/// macOS presents this once per app identity and silently ignores later calls,
/// so onboarding pairs it with a link to System Settings.
pub fn request_accessibility_access() -> PermissionStatus {
    ax::prompt_for_trust();
    accessibility_status()
}

/// Open the Accessibility pane in System Settings.
pub fn open_accessibility_settings() {
    open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility");
}

/// Open the Microphone pane in System Settings.
pub fn open_microphone_settings() {
    open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone");
}

fn open_url(url: &str) {
    if let Err(error) = std::process::Command::new("open").arg(url).spawn() {
        tracing::warn!(?error, url, "could not open System Settings");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_permissions_is_side_effect_free() {
        // Reading must never trigger a prompt; calling it repeatedly in a test
        // run would be unbearable if it did.
        let first = snapshot();
        let second = snapshot();
        assert_eq!(first.microphone, second.microphone);
        assert_eq!(first.accessibility, second.accessibility);
    }

    #[test]
    fn readiness_is_derived_from_the_two_permissions() {
        let granted = PermissionSnapshot {
            microphone: PermissionStatus::Granted,
            accessibility: PermissionStatus::Granted,
            // Not part of readiness: only Apple Speech needs it, so a machine
            // without it is still ready to dictate with every other engine.
            speech_recognition: PermissionStatus::NotDetermined,
        };
        assert!(granted.can_capture() && granted.can_insert());

        let mic_only = PermissionSnapshot {
            microphone: PermissionStatus::Granted,
            accessibility: PermissionStatus::NotDetermined,
            speech_recognition: PermissionStatus::NotDetermined,
        };
        assert!(mic_only.can_capture());
        assert!(!mic_only.can_insert());
    }
}
