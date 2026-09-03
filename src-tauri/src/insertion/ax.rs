//! Thin, safe-ish wrappers over the macOS Accessibility C API.
//!
//! Clide only needs a handful of AX calls, so it binds them directly rather
//! than taking a dependency that wraps the whole framework. Every element is
//! owned by `AXElement`, which releases it on drop.

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::c_void;

use core_foundation::base::{CFGetTypeID, CFRelease, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::{CFString, CFStringGetTypeID, CFStringRef};

pub type AXUIElementRef = *mut c_void;
pub type AXError = i32;

pub const kAXErrorSuccess: AXError = 0;
/// Returned by every AX call when this process is not trusted. Its *absence*
/// is what proves the grant, which is more reliable than asking for the flag.
pub const kAXErrorAPIDisabled: AXError = -25211;

// Attribute names, as the framework spells them.
pub const ATTR_FOCUSED_UI_ELEMENT: &str = "AXFocusedUIElement";
pub const ATTR_FOCUSED_APPLICATION: &str = "AXFocusedApplication";
pub const ATTR_SELECTED_TEXT: &str = "AXSelectedText";
pub const ATTR_TITLE: &str = "AXTitle";
pub const ATTR_ROLE: &str = "AXRole";

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> u8;
    fn AXIsProcessTrustedWithOptions(options: CFTypeRef) -> u8;
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    fn AXUIElementIsAttributeSettable(
        element: AXUIElementRef,
        attribute: CFStringRef,
        settable: *mut u8,
    ) -> AXError;
    fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> AXError;

    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

/// Whether this process is allowed to drive other applications.
///
/// Without this, every AX call returns `kAXErrorAPIDisabled` and insertion
/// falls through to the clipboard path.
///
/// # Why this does not simply return `AXIsProcessTrusted()`
///
/// That flag is resolved when the process first asks and can then stay stale
/// for the lifetime of the process. Granting Accessibility while Clide is
/// running — which is exactly what onboarding asks people to do — leaves it
/// reporting `false` until the app is relaunched, so the user sees the switch
/// on in System Settings while Clide insists it is off.
///
/// So the flag is only the fast path. When it says no, Clide *tries an
/// Accessibility call* and looks at the error: anything other than
/// `kAXErrorAPIDisabled` means the API is answering us, which is the thing the
/// permission actually governs. Asking by doing cannot go stale.
pub fn is_process_trusted() -> bool {
    if unsafe { AXIsProcessTrusted() != 0 } {
        return true;
    }
    accessibility_api_responds()
}

/// Probe the Accessibility API and report whether it answered at all.
///
/// The *value* is irrelevant — "nothing is focused" is a perfectly good answer
/// from a trusted process. Only `kAXErrorAPIDisabled` means untrusted.
fn accessibility_api_responds() -> bool {
    unsafe {
        let system = AXUIElementCreateSystemWide();
        if system.is_null() {
            return false;
        }

        let attribute = CFString::from_static_string("AXFocusedApplication");
        let mut value: CFTypeRef = std::ptr::null();
        let status =
            AXUIElementCopyAttributeValue(system, attribute.as_concrete_TypeRef(), &mut value);

        if !value.is_null() {
            CFRelease(value);
        }
        CFRelease(system as CFTypeRef);

        status != kAXErrorAPIDisabled
    }
}

/// Ask macOS to show its "allow Accessibility" dialog.
///
/// The system only presents this once per app identity; afterwards it is a
/// silent no-op, which is why onboarding also offers to open System Settings.
pub fn prompt_for_trust() -> bool {
    unsafe {
        let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
        let options = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::true_value())]);
        AXIsProcessTrustedWithOptions(options.as_CFTypeRef()) != 0
    }
}

/// An owned `AXUIElementRef`.
pub struct AXElement(AXUIElementRef);

impl AXElement {
    /// The system-wide element: the entry point for "whatever is focused".
    pub fn system_wide() -> Option<Self> {
        let element = unsafe { AXUIElementCreateSystemWide() };
        (!element.is_null()).then_some(Self(element))
    }

    /// The Accessibility root for one application. This lets insertion use
    /// the app that owned focus when dictation started rather than whichever
    /// process happens to be frontmost after transcription finishes.
    pub fn application(pid: i32) -> Option<Self> {
        let element = unsafe { AXUIElementCreateApplication(pid) };
        (!element.is_null()).then_some(Self(element))
    }

    /// Read an attribute that is itself an element (focused control, focused app).
    pub fn element_attribute(&self, attribute: &str) -> Option<Self> {
        let value = self.copy_attribute(attribute)?;
        Some(Self(value as AXUIElementRef))
    }

    /// Read a string attribute, returning `None` when it is absent or of
    /// another type.
    pub fn string_attribute(&self, attribute: &str) -> Option<String> {
        let value = self.copy_attribute(attribute)?;
        unsafe {
            if CFGetTypeID(value) != CFStringGetTypeID() {
                CFRelease(value);
                return None;
            }
            let string = CFString::wrap_under_create_rule(value as CFStringRef);
            Some(string.to_string())
        }
    }

    fn copy_attribute(&self, attribute: &str) -> Option<CFTypeRef> {
        let name = CFString::new(attribute);
        let mut value: CFTypeRef = std::ptr::null();
        let status = unsafe {
            AXUIElementCopyAttributeValue(self.0, name.as_concrete_TypeRef(), &mut value)
        };
        (status == kAXErrorSuccess && !value.is_null()).then_some(value)
    }

    /// Whether this element will accept a write to `attribute`.
    ///
    /// Checked before writing so a read-only control (a web page, a label)
    /// falls back to pasting instead of silently swallowing the transcript.
    pub fn is_settable(&self, attribute: &str) -> bool {
        let name = CFString::new(attribute);
        let mut settable: u8 = 0;
        let status = unsafe {
            AXUIElementIsAttributeSettable(self.0, name.as_concrete_TypeRef(), &mut settable)
        };
        status == kAXErrorSuccess && settable != 0
    }

    /// Write a string attribute. `Ok(())` means the framework accepted it.
    pub fn set_string_attribute(&self, attribute: &str, value: &str) -> Result<(), AXError> {
        let name = CFString::new(attribute);
        let text = CFString::new(value);
        let status = unsafe {
            AXUIElementSetAttributeValue(self.0, name.as_concrete_TypeRef(), text.as_CFTypeRef())
        };
        if status == kAXErrorSuccess {
            Ok(())
        } else {
            Err(status)
        }
    }

    pub fn pid(&self) -> Option<i32> {
        let mut pid: i32 = 0;
        let status = unsafe { AXUIElementGetPid(self.0, &mut pid) };
        (status == kAXErrorSuccess).then_some(pid)
    }
}

impl Drop for AXElement {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0 as CFTypeRef) };
        }
    }
}

/// Human-readable name for the AX error codes Clide can actually hit.
pub fn describe(status: AXError) -> String {
    match status {
        0 => "success".into(),
        -25200 => "the application refused the request".into(),
        -25201 => "the value was not accepted".into(),
        -25202 => "the application is not responding".into(),
        -25204 => "the focused control has no editable text".into(),
        -25205 => "the focused control does not accept text".into(),
        -25211 => "Accessibility access is not enabled for Clide".into(),
        -25212 => "there is nothing focused to type into".into(),
        other => format!("Accessibility error {other}"),
    }
}
