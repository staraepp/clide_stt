//! Getting the transcript into the application the user was speaking to.
//!
//! Two strategies, tried in order:
//!
//! 1. **Clipboard** — every completed transcript is copied and remains there.
//! 2. **Accessibility** — write into the control that was focused when
//!    dictation began.
//! 3. **Typing** — when a control refuses direct Accessibility writes (common
//!    in web views), send the text itself as Unicode keystrokes.
//! 4. **Clipboard paste** — last resort, for anything that ignores synthetic
//!    typing but honours a paste chord.
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

    // Log where the text is actually going. When insertion silently misses,
    // the first question is always "was the intended app still frontmost?".
    tracing::debug!(
        target_app = target.app_name.as_deref().unwrap_or("unknown"),
        target_pid = target.pid,
        frontmost = focus::frontmost().app_name.as_deref().unwrap_or("unknown"),
        "inserting"
    );

    match insert_via_accessibility(text, target) {
        Ok(()) => return Ok(InsertionMethod::Accessibility),
        Err(reason) => tracing::debug!(reason, "accessibility insertion declined; typing"),
    }

    // Typing before pasting. Cmd+V is a *command* the target app has to
    // recognise and act on, and Electron/Chromium apps reject synthetic chords
    // that do not match what they expect — the Claude app took neither the
    // Accessibility write nor the paste. Unicode keystrokes carry the text
    // itself, so there is no chord to refuse.
    match clipboard::type_text(text) {
        Ok(()) => return Ok(InsertionMethod::Typed),
        Err(reason) => tracing::debug!(reason, "typing failed; falling back to paste"),
    }

    match insert_via_paste(text) {
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
    // The system-wide AX root is macOS's canonical source for the actual caret.
    // Prefer it while the same process still owns focus; fall back to the app
    // root if focus briefly moved during finalization.
    let focused = ax::AXElement::system_wide()
        .and_then(|root| root.element_attribute(ax::ATTR_FOCUSED_UI_ELEMENT))
        .filter(|element| target.pid.is_none() || element.pid() == target.pid)
        .or_else(|| {
            target.pid.and_then(|pid| {
                ax::AXElement::application(pid)
                    .and_then(|root| root.element_attribute(ax::ATTR_FOCUSED_UI_ELEMENT))
            })
        })
        .ok_or_else(|| "nothing is focused to type into in the target application".to_string())?;

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
fn insert_via_paste(text: &str) -> Result<(), String> {
    clipboard::access(|pasteboard| {
        if !pasteboard.set_text(text) {
            return Err("the clipboard could not be written".into());
        }

        if pasteboard.text().as_deref() != Some(text) {
            return Err("the clipboard did not retain the complete transcript".into());
        }

        clipboard::wait_until_pasteboard_is_ready();
        clipboard::send_paste_keystroke()
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
    /// after focusing TextEdit. Set `CLIDE_INSERT_TEST_PID` to exercise direct
    /// Accessibility insertion against a specific editor process.
    #[test]
    #[ignore = "types into the focused application; run manually"]
    fn insertion_reaches_the_focused_app() {
        let target = std::env::var("CLIDE_INSERT_TEST_PID")
            .ok()
            .and_then(|pid| pid.parse::<i32>().ok())
            .map(|pid| FocusTarget {
                app_name: Some("integration target".into()),
                bundle_id: None,
                pid: Some(pid),
            })
            .unwrap_or_else(focus::frontmost);
        let text = std::env::var("CLIDE_INSERT_TEST_TEXT")
            .unwrap_or_else(|_| "Clide insertion test.".into());
        let method = insert(&text, &target).expect("insertion failed");
        eprintln!("inserted into {} via {:?}", target.label(), method);
    }

    /// Verifies the paste **actually landed**, by reading the focused
    /// control back through Accessibility rather than trusting that posting
    /// the event succeeded. Posting always "succeeds"; that was the whole
    /// problem.
    ///
    /// Run with TextEdit (or any AX-readable editor) focused and empty:
    ///   `cargo test -- --ignored paste_lands_in_the_focused_control`
    #[test]
    #[ignore = "pastes into the frontmost application; run manually"]
    fn paste_lands_in_the_focused_control() {
        let marker = format!("clide-paste-{}", std::process::id());

        clipboard::access(|pasteboard| {
            assert!(pasteboard.set_text(&marker));
            Ok::<(), String>(())
        })
        .unwrap();

        clipboard::wait_until_pasteboard_is_ready();
        clipboard::send_paste_keystroke().expect("posting the paste failed");

        // Give the target a moment to process the keystroke.
        std::thread::sleep(std::time::Duration::from_millis(400));

        let focused = ax::AXElement::system_wide()
            .and_then(|root| root.element_attribute(ax::ATTR_FOCUSED_UI_ELEMENT))
            .expect("nothing is focused — focus a text editor before running this");

        let value = focused
            .string_attribute(ax::ATTR_VALUE)
            .unwrap_or_default();

        assert!(
            value.contains(&marker),
            "the paste did not reach the focused control. It contains: {value:?}"
        );
    }

    /// Exercises the WebKit-compatible fallback directly. The target editor
    /// must be frontmost because HID events follow the real keyboard focus.
    #[test]
    #[ignore = "pastes into the frontmost application; run manually"]
    fn clipboard_paste_reaches_the_focused_app() {
        assert!(ax::is_process_trusted(), "Accessibility is not available");
        let text = std::env::var("CLIDE_INSERT_TEST_TEXT")
            .unwrap_or_else(|_| "Clide clipboard paste test.".into());
        insert_via_paste(&text).expect("clipboard paste failed");
        assert_eq!(clipboard::text().as_deref(), Some(text.as_str()));
    }
}
