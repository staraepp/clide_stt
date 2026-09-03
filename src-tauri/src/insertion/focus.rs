//! Which application is about to receive the transcript.
//!
//! Read from `NSWorkspace` rather than the Accessibility API so Clide can
//! label a history entry with the right app even before Accessibility access
//! has been granted. This is context level 1 in the blueprint's terms —
//! application identity only, never window or document contents.

use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};
use objc2::rc::autoreleasepool;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};

use serde::Serialize;

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusTarget {
    /// Display name, e.g. "TextEdit". Stored as `source_app` in history.
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
    /// Process that owned focus when dictation began. Kept internal so the
    /// eventual paste can be delivered to the original app even if another
    /// Clide window briefly becomes active while transcription is running.
    #[serde(skip)]
    pub pid: Option<i32>,
}

impl FocusTarget {
    pub fn label(&self) -> &str {
        self.app_name.as_deref().unwrap_or("Unknown")
    }

    /// True when Clide itself is frontmost — the case during the onboarding
    /// test dictation, and worth distinguishing in the UI.
    pub fn is_clide(&self) -> bool {
        self.bundle_id.as_deref() == Some("com.staraep.clide")
    }
}

unsafe fn ns_string_to_rust(ns: *mut AnyObject) -> Option<String> {
    if ns.is_null() {
        return None;
    }
    Some(CFString::wrap_under_get_rule(ns as CFStringRef).to_string())
}

/// The frontmost application right now.
pub fn frontmost() -> FocusTarget {
    autoreleasepool(|_| unsafe {
        let workspace: *mut AnyObject = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace.is_null() {
            return FocusTarget::default();
        }
        let app: *mut AnyObject = msg_send![workspace, frontmostApplication];
        if app.is_null() {
            return FocusTarget::default();
        }

        let name: *mut AnyObject = msg_send![app, localizedName];
        let bundle: *mut AnyObject = msg_send![app, bundleIdentifier];
        let pid: i32 = msg_send![app, processIdentifier];

        FocusTarget {
            app_name: ns_string_to_rust(name),
            bundle_id: ns_string_to_rust(bundle),
            pid: (pid > 0).then_some(pid),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_the_frontmost_application_does_not_crash_headless() {
        // In CI there may be no frontmost app; the contract is that this
        // degrades to `None` rather than panicking or blocking.
        let target = frontmost();
        assert!(!target.label().is_empty());
    }
}
