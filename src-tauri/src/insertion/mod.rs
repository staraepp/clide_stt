//! Getting the transcript into the application the user was speaking to.
//!
//! Two strategies, tried in order:
//!
//! 1. **Accessibility** — write the text into the focused control directly.
//!    Nothing is touched except the caret position, and the clipboard is left
//!    completely alone.
//! 2. **Clipboard paste** — put the text on the pasteboard, send Cmd+V, then
//!    put the previous clipboard back.
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

/// Insert `text` wherever the user is currently typing.
///
/// Blocking: it talks to the Accessibility API and sleeps while the target
/// application services a paste. Call it from `spawn_blocking`.
pub fn insert(text: &str) -> Result<InsertionMethod, InsertionFailure> {
    if text.is_empty() {
        return Err(InsertionFailure {
            message: "There was nothing to insert.".into(),
            on_clipboard: false,
        });
    }

    // Without Accessibility access neither strategy can work: direct writes
    // are refused and synthetic keystrokes are discarded silently. Skip
    // straight to leaving the text somewhere the user can reach it.
    if !ax::is_process_trusted() {
        let on_clipboard = clipboard::set_text(text);
        return Err(InsertionFailure {
            message: "clide needs Accessibility access to type into other apps.".into(),
            on_clipboard,
        });
    }

    match insert_via_accessibility(text) {
        Ok(()) => return Ok(InsertionMethod::Accessibility),
        Err(reason) => tracing::debug!(reason, "accessibility insertion declined; pasting"),
    }

    match insert_via_paste(text) {
        Ok(()) => Ok(InsertionMethod::ClipboardPaste),
        Err(reason) => {
            // Last resort: the transcript stays on the clipboard and the UI
            // offers Copy.
            let on_clipboard = clipboard::set_text(text);
            Err(InsertionFailure {
                message: reason,
                on_clipboard,
            })
        }
    }
}

/// Write into the focused control's selection, which is what "type at the
/// caret" means in Accessibility terms.
fn insert_via_accessibility(text: &str) -> Result<(), String> {
    let system = ax::AXElement::system_wide()
        .ok_or_else(|| "the Accessibility system element is unavailable".to_string())?;

    let focused = system
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

/// Borrow the clipboard, paste, and give it back.
///
/// The pasteboard lock is held across the whole sequence so a Copy elsewhere
/// in the app cannot land between the borrow and the restore and get undone.
fn insert_via_paste(text: &str) -> Result<(), String> {
    clipboard::access(|pasteboard| {
        let previous = pasteboard.snapshot();

        if !pasteboard.set_text(text) {
            return Err("the clipboard could not be written".into());
        }

        if let Err(error) = clipboard::send_paste_keystroke() {
            // Put the clipboard back before reporting: the user should not
            // lose what they had copied because our keystroke failed.
            pasteboard.restore(previous);
            return Err(error);
        }

        // Give the target application time to read the pasteboard before the
        // original contents go back.
        std::thread::sleep(clipboard::PASTE_SETTLE);
        pasteboard.restore(previous);

        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_is_refused_rather_than_pasted() {
        let failure = insert("").unwrap_err();
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
        let method = insert("Clide insertion test.").expect("insertion failed");
        eprintln!("inserted into {} via {:?}", target.label(), method);
    }
}
