//! Getting the transcript into the application the user was speaking to.
//!
//! Two strategies, tried in order:
//!
//! 1. **Clipboard** — every completed transcript is copied and remains there.
//! 2. **Accessibility** — write into the control that was focused when
//!    dictation began.
//! 3. **Clipboard paste** — when a control refuses direct Accessibility writes
//!    (common in web views), send Cmd+V directly to the captured app process.
//!
//! If both fail the transcript is *still* left on the clipboard, because a
//! transcript that reached this point is a success that insertion must not be
//! allowed to destroy.

pub mod ax;
pub mod clipboard;
pub mod focus;

use crate::dictation::machine::InsertionMethod;

pub use focus::FocusTarget;

#[derive(Debug)]
pub struct InsertionFailure {
    /// Shown to the user; explains what to do next.
    pub message: String,
    /// Whether the transcript is at least sitting on the clipboard.
    pub on_clipboard: bool,
}

/// Insert `text` into the application captured when dictation began.
///
/// Blocking: it talks to the pasteboard and Accessibility APIs. Call it from
/// `spawn_blocking`.
pub fn insert(text: &str, target: &FocusTarget) -> Result<InsertionMethod, InsertionFailure> {
    if text.is_empty() {
        return Err(InsertionFailure {
            message: "There was nothing to insert.".into(),
            on_clipboard: false,
        });
    }

    // Copy first and deliberately keep the transcript there. Besides matching
    // the user's explicit preference, this is the recovery path if any form of
    // insertion fails.
    let on_clipboard = clipboard::set_text(text);
    if !on_clipboard {
        return Err(InsertionFailure {
            message: "The transcript was ready, but the clipboard could not be written.".into(),
            on_clipboard: false,
        });
    }

    // Without Accessibility access neither insertion strategy can work: direct writes
    // are refused and synthetic keystrokes are discarded silently. Skip
    // straight to leaving the text somewhere the user can reach it.
    if !ax::is_process_trusted() {
        return Err(InsertionFailure {
            message: "clide needs Accessibility access to type into other apps.".into(),
            on_clipboard,
        });
    }

    match insert_via_accessibility(text, target) {
        Ok(()) => return Ok(InsertionMethod::Accessibility),
        Err(reason) => tracing::debug!(reason, "accessibility insertion declined; pasting"),
    }

    match insert_via_paste(text, target.pid) {
        Ok(()) => Ok(InsertionMethod::ClipboardPaste),
        Err(reason) => Err(InsertionFailure {
            message: reason,
            on_clipboard,
        }),
    }
}

/// Write into the focused control's selection, which is what "type at the
/// caret" means in Accessibility terms.
fn insert_via_accessibility(text: &str, target: &FocusTarget) -> Result<(), String> {
    let root = match target.pid {
        Some(pid) => ax::AXElement::application(pid).ok_or_else(|| {
            "the target application's Accessibility element is unavailable".to_string()
        })?,
        None => ax::AXElement::system_wide()
            .ok_or_else(|| "the Accessibility system element is unavailable".to_string())?,
    };

    let focused = root
        .element_attribute(ax::ATTR_FOCUSED_UI_ELEMENT)
        .ok_or_else(|| "nothing is focused to type into".to_string())?;

    // Read-only controls (web views, labels, canvas-based editors) report the
    // attribute but refuse writes. Asking first avoids a silent no-op.
    if !focused.is_settable(ax::ATTR_SELECTED_TEXT) {
        return Err("the focused control does not accept direct text".into());
    }

    focused
        .set_string_attribute(ax::ATTR_SELECTED_TEXT, text)
        .map_err(ax::describe)
}

/// Put the transcript on the clipboard and send a targeted paste. It stays on
/// the clipboard afterward by design, so slow web views can consume it on
/// their own schedule and the user can paste it manually if needed.
fn insert_via_paste(text: &str, target_pid: Option<i32>) -> Result<(), String> {
    clipboard::access(|pasteboard| {
        if !pasteboard.set_text(text) {
            return Err("the clipboard could not be written".into());
        }

        clipboard::send_paste_keystroke(target_pid)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_is_refused_rather_than_pasted() {
        let failure = insert("", &FocusTarget::default()).unwrap_err();
        assert!(!failure.on_clipboard);
    }

    /// The real end-to-end check. Ignored by default because it types into
    /// whatever is focused and needs Accessibility access.
    ///
    /// Run with: `cargo test -- --ignored insertion_reaches_the_focused_app`
    /// after focusing TextEdit.
    #[test]
    #[ignore = "types into the focused application; run manually"]
    fn insertion_reaches_the_focused_app() {
        let target = focus::frontmost();
        let method = insert("Clide insertion test.", &target).expect("insertion failed");
        eprintln!("inserted into {} via {:?}", target.label(), method);
    }
}
