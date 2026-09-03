//! Microphone authorisation via AVFoundation.

use std::sync::mpsc::sync_channel;
use std::time::Duration;

use block2::RcBlock;
use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use objc2::rc::autoreleasepool;
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send};

use super::PermissionStatus;

// Linked for `AVCaptureDevice`; the class is resolved at runtime.
#[link(name = "AVFoundation", kind = "framework")]
extern "C" {}

/// `AVMediaTypeAudio`. The constant is a plain NSString whose value is the
/// four-character code, so it can be built without importing the symbol.
const MEDIA_TYPE_AUDIO: &str = "soun";

/// `AVAuthorizationStatus`.
const STATUS_NOT_DETERMINED: isize = 0;
const STATUS_RESTRICTED: isize = 1;
const STATUS_DENIED: isize = 2;
const STATUS_AUTHORIZED: isize = 3;

/// The user cannot be waited on forever; if the prompt is left open we report
/// the current (still undetermined) state rather than hanging the caller.
const PROMPT_TIMEOUT: Duration = Duration::from_secs(120);

fn media_type() -> CFString {
    CFString::new(MEDIA_TYPE_AUDIO)
}

/// Read the current authorisation without prompting.
pub fn status() -> PermissionStatus {
    autoreleasepool(|_| unsafe {
        let audio = media_type();
        let raw: isize = msg_send![
            class!(AVCaptureDevice),
            authorizationStatusForMediaType: audio.as_concrete_TypeRef() as *mut AnyObject
        ];
        match raw {
            STATUS_AUTHORIZED => PermissionStatus::Granted,
            STATUS_DENIED => PermissionStatus::Denied,
            STATUS_RESTRICTED => PermissionStatus::Restricted,
            STATUS_NOT_DETERMINED => PermissionStatus::NotDetermined,
            other => {
                tracing::warn!(other, "unknown AVAuthorizationStatus");
                PermissionStatus::NotDetermined
            }
        }
    })
}

/// Show the system microphone prompt and wait for the answer.
///
/// Blocking: run it on a blocking thread. macOS only presents the dialog when
/// the status is `NotDetermined`; once denied, the answer comes back
/// immediately and the caller should send the user to System Settings.
pub fn request_microphone_access() -> PermissionStatus {
    let current = status();
    if current != PermissionStatus::NotDetermined {
        return current;
    }

    let (tx, rx) = sync_channel::<bool>(1);
    let completion = RcBlock::new(move |granted: Bool| {
        let _ = tx.send(granted.as_bool());
    });

    autoreleasepool(|_| unsafe {
        let audio = media_type();
        let _: () = msg_send![
            class!(AVCaptureDevice),
            requestAccessForMediaType: audio.as_concrete_TypeRef() as *mut AnyObject,
            completionHandler: &*completion
        ];
    });

    match rx.recv_timeout(PROMPT_TIMEOUT) {
        Ok(_) => status(),
        Err(_) => {
            tracing::warn!("microphone prompt was not answered in time");
            status()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_returns_a_known_value() {
        // Whatever this machine's state is, it must map to a real variant
        // rather than panicking on an unexpected code.
        let value = status();
        assert!(matches!(
            value,
            PermissionStatus::Granted
                | PermissionStatus::Denied
                | PermissionStatus::NotDetermined
                | PermissionStatus::Restricted
        ));
    }

    #[test]
    fn an_already_answered_prompt_is_not_shown_again() {
        // When the status is settled, requesting must return immediately with
        // that same status and never open a dialog.
        if status() != PermissionStatus::NotDetermined {
            let before = status();
            assert_eq!(request_microphone_access(), before);
        }
    }
}
