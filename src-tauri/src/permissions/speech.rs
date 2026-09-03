//! Speech recognition permission.
//!
//! Separate from the microphone, and required even though Apple's recogniser
//! runs on-device: macOS treats handing audio to the recogniser as its own
//! consent. Without it, Apple Speech fails on every attempt with a status of
//! `NotDetermined` — which is exactly the bug this module exists to close.
//!
//! Only relevant when Apple Speech is the selected provider. Every other engine
//! never touches it, so Clide never asks.

use objc2_speech::SFSpeechRecognizerAuthorizationStatus;

use super::PermissionStatus;

pub fn status() -> PermissionStatus {
    map(crate::providers::apple::authorization())
}

/// Ask macOS, blocking until the user answers.
///
/// Safe to call when already decided: the underlying request returns the
/// existing answer without showing a dialog.
pub fn request() -> PermissionStatus {
    map(crate::providers::apple::request_authorization())
}

fn map(status: SFSpeechRecognizerAuthorizationStatus) -> PermissionStatus {
    match status {
        SFSpeechRecognizerAuthorizationStatus::Authorized => PermissionStatus::Granted,
        SFSpeechRecognizerAuthorizationStatus::Denied => PermissionStatus::Denied,
        SFSpeechRecognizerAuthorizationStatus::Restricted => PermissionStatus::Restricted,
        // Includes NotDetermined and any status a future macOS adds.
        _ => PermissionStatus::NotDetermined,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_the_status_does_not_panic() {
        let _ = status();
    }

    /// An unrecognised status must read as "not asked yet", never as granted —
    /// optimism here would mean a failed dictation instead of a prompt.
    #[test]
    fn unknown_states_are_never_treated_as_granted() {
        assert_eq!(
            map(SFSpeechRecognizerAuthorizationStatus::NotDetermined),
            PermissionStatus::NotDetermined
        );
        assert_eq!(
            map(SFSpeechRecognizerAuthorizationStatus::Denied),
            PermissionStatus::Denied
        );
    }
}
