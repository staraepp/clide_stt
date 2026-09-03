//! Clipboard access and the synthetic paste used as the insertion fallback.
//!
//! Every completed transcript is copied to the clipboard and left there. This
//! gives the user a reliable recovery path and prevents a slow target app from
//! missing a paste because Clide restored the previous contents too early.
//!
//! **All pasteboard access is serialised.** `NSPasteboard` maintains an
//! internal type cache that is not thread-safe: concurrent `types` calls
//! corrupt it and crash inside AppKit. Clide reaches the clipboard from the
//! insertion worker and from the Copy command, so every entry point goes
//! through [`access`], and the borrow-paste-restore sequence holds the lock
//! for its whole duration so nothing can interleave with it.

use std::ffi::c_void;
use std::sync::{Mutex, MutexGuard};

use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};
use objc2::rc::autoreleasepool;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};

use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

/// Virtual key codes (Carbon `kVK_*`), stable across keyboard layouts.
const KEY_V: u16 = 0x09;
const KEY_COMMAND: u16 = 0x37;

static PASTEBOARD: Mutex<()> = Mutex::new(());

/// Proof that the caller holds the pasteboard lock.
///
/// Every operation hangs off this token, which makes it impossible to touch
/// `NSPasteboard` without having serialised first.
pub struct Pasteboard {
    _guard: MutexGuard<'static, ()>,
}

/// Run `work` with exclusive access to the system pasteboard.
pub fn access<R>(work: impl FnOnce(&Pasteboard) -> R) -> R {
    let guard = PASTEBOARD.lock().unwrap_or_else(|e| e.into_inner());
    let pasteboard = Pasteboard { _guard: guard };
    work(&pasteboard)
}

/// Everything that was on the pasteboard, by UTI.
pub struct ClipboardSnapshot {
    items: Vec<(String, Vec<u8>)>,
}

impl ClipboardSnapshot {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn empty() -> Self {
        Self { items: Vec::new() }
    }
}

/// NSString -> String via the CFString toll-free bridge.
unsafe fn ns_string_to_rust(ns: *mut AnyObject) -> Option<String> {
    if ns.is_null() {
        return None;
    }
    Some(CFString::wrap_under_get_rule(ns as CFStringRef).to_string())
}

/// String -> NSString, likewise.
fn ns(value: &str) -> CFString {
    CFString::new(value)
}

unsafe fn general_pasteboard() -> *mut AnyObject {
    msg_send![class!(NSPasteboard), generalPasteboard]
}

impl Pasteboard {
    /// Capture the current contents so they can be restored.
    pub fn snapshot(&self) -> ClipboardSnapshot {
        autoreleasepool(|_| unsafe {
            let pasteboard = general_pasteboard();
            if pasteboard.is_null() {
                return ClipboardSnapshot::empty();
            }

            let types: *mut AnyObject = msg_send![pasteboard, types];
            if types.is_null() {
                return ClipboardSnapshot::empty();
            }

            let count: usize = msg_send![types, count];
            let mut items = Vec::with_capacity(count);

            for index in 0..count {
                let uti: *mut AnyObject = msg_send![types, objectAtIndex: index];
                let Some(uti_string) = ns_string_to_rust(uti) else {
                    continue;
                };

                let data: *mut AnyObject = msg_send![pasteboard, dataForType: uti];
                if data.is_null() {
                    // Promised data the owning app has not produced. It cannot
                    // be restored, so it is not worth capturing.
                    continue;
                }

                let length: usize = msg_send![data, length];
                let bytes: *const c_void = msg_send![data, bytes];
                if bytes.is_null() || length == 0 {
                    continue;
                }
                items.push((
                    uti_string,
                    std::slice::from_raw_parts(bytes as *const u8, length).to_vec(),
                ));
            }

            ClipboardSnapshot { items }
        })
    }

    /// Put a snapshot back, replacing whatever is on the pasteboard now.
    pub fn restore(&self, snapshot: ClipboardSnapshot) {
        if snapshot.is_empty() {
            return;
        }
        autoreleasepool(|_| unsafe {
            let pasteboard = general_pasteboard();
            if pasteboard.is_null() {
                return;
            }
            let _: isize = msg_send![pasteboard, clearContents];

            for (uti, bytes) in &snapshot.items {
                let uti_string = ns(uti);
                let data: *mut AnyObject = msg_send![
                    class!(NSData),
                    dataWithBytes: bytes.as_ptr() as *const c_void,
                    length: bytes.len()
                ];
                if data.is_null() {
                    continue;
                }
                let _: bool = msg_send![
                    pasteboard,
                    setData: data,
                    forType: uti_string.as_concrete_TypeRef() as *mut AnyObject
                ];
            }
        });
    }

    /// Replace the pasteboard with a single plain-text item.
    pub fn set_text(&self, text: &str) -> bool {
        autoreleasepool(|_| unsafe {
            let pasteboard = general_pasteboard();
            if pasteboard.is_null() {
                return false;
            }
            let _: isize = msg_send![pasteboard, clearContents];

            let value = ns(text);
            let uti = ns("public.utf8-plain-text");
            msg_send![
                pasteboard,
                setString: value.as_concrete_TypeRef() as *mut AnyObject,
                forType: uti.as_concrete_TypeRef() as *mut AnyObject
            ]
        })
    }

    /// Read the plain-text representation, if there is one.
    pub fn text(&self) -> Option<String> {
        autoreleasepool(|_| unsafe {
            let pasteboard = general_pasteboard();
            if pasteboard.is_null() {
                return None;
            }
            let uti = ns("public.utf8-plain-text");
            let value: *mut AnyObject = msg_send![
                pasteboard,
                stringForType: uti.as_concrete_TypeRef() as *mut AnyObject
            ];
            ns_string_to_rust(value)
        })
    }
}

/// Replace the clipboard with `text`. Used by every Copy affordance.
pub fn set_text(text: &str) -> bool {
    access(|pasteboard| pasteboard.set_text(text))
}

/// Read the clipboard's plain text.
pub fn text() -> Option<String> {
    access(|pasteboard| pasteboard.text())
}

/// Send Cmd+V to the app that owned focus when dictation began.
///
/// The Command key is pressed and released as real key events rather than only
/// setting the modifier flag: some applications watch for the physical
/// modifier and ignore a bare flagged keystroke.
pub fn send_paste_keystroke(target_pid: Option<i32>) -> Result<(), String> {
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|_| "could not create a keyboard event source".to_string())?;

    let events = [
        (KEY_COMMAND, true, CGEventFlags::CGEventFlagCommand),
        (KEY_V, true, CGEventFlags::CGEventFlagCommand),
        (KEY_V, false, CGEventFlags::CGEventFlagCommand),
        (KEY_COMMAND, false, CGEventFlags::CGEventFlagNull),
    ];

    for (key, down, flags) in events {
        let event = CGEvent::new_keyboard_event(source.clone(), key, down)
            .map_err(|_| "could not create the paste keystroke".to_string())?;
        event.set_flags(flags);
        match target_pid {
            Some(pid) => event.post_to_pid(pid),
            None => event.post(CGEventTapLocation::HID),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_survives_a_snapshot_and_restore_round_trip() {
        access(|pasteboard| {
            let original = pasteboard.snapshot();

            assert!(pasteboard.set_text("clide round trip"));
            assert_eq!(pasteboard.text().as_deref(), Some("clide round trip"));

            let borrowed = pasteboard.snapshot();
            assert!(pasteboard.set_text("something else"));
            assert_eq!(pasteboard.text().as_deref(), Some("something else"));

            pasteboard.restore(borrowed);
            assert_eq!(
                pasteboard.text().as_deref(),
                Some("clide round trip"),
                "restoring the snapshot did not bring the clipboard back"
            );

            pasteboard.restore(original);
        });
    }

    #[test]
    fn restoring_an_empty_snapshot_leaves_the_clipboard_alone() {
        access(|pasteboard| {
            let original = pasteboard.snapshot();

            pasteboard.set_text("untouched");
            pasteboard.restore(ClipboardSnapshot::empty());
            assert_eq!(pasteboard.text().as_deref(), Some("untouched"));

            pasteboard.restore(original);
        });
    }

    /// The crash this lock exists to prevent: AppKit's pasteboard type cache
    /// is not thread-safe, and Clide reaches the clipboard from more than one
    /// thread.
    #[test]
    fn concurrent_access_is_serialised() {
        let threads: Vec<_> = (0..8)
            .map(|i| {
                std::thread::spawn(move || {
                    for _ in 0..12 {
                        let captured = access(|pasteboard| {
                            let before = pasteboard.snapshot();
                            pasteboard.set_text(&format!("thread {i}"));
                            let seen = pasteboard.text();
                            pasteboard.restore(before);
                            seen
                        });
                        // Under the lock, a thread always reads back its own
                        // write; without it, this is where the data races.
                        assert_eq!(captured.as_deref(), Some(format!("thread {i}").as_str()));
                    }
                })
            })
            .collect();

        for thread in threads {
            thread.join().expect("a clipboard thread crashed");
        }
    }
}
