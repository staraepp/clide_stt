//! Windows desktop insertion. Never sends text to a different foreground process.
use crate::dictation::machine::InsertionMethod;
use std::{
    ffi::c_void,
    sync::Mutex,
    time::{Duration, Instant},
};
type Handle = *mut c_void;
#[repr(C)]
struct Keyboard {
    vk: u16,
    scan: u16,
    flags: u32,
    time: u32,
    extra: usize,
}
#[repr(C)]
union Payload {
    keyboard: std::mem::ManuallyDrop<Keyboard>,
    padding: [u64; 4],
}
#[repr(C)]
struct Input {
    kind: u32,
    payload: Payload,
}
#[link(name = "user32")]
extern "system" {
    fn GetForegroundWindow() -> Handle;
    fn GetWindowThreadProcessId(w: Handle, p: *mut u32) -> u32;
    fn SendInput(n: u32, p: *const Input, size: i32) -> u32;
    fn GetAsyncKeyState(key: i32) -> i16;
    fn OpenClipboard(w: Handle) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn SetClipboardData(format: u32, h: Handle) -> Handle;
}
#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(flags: u32, size: usize) -> Handle;
    fn GlobalLock(h: Handle) -> Handle;
    fn GlobalUnlock(h: Handle) -> i32;
    fn GlobalFree(h: Handle) -> Handle;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn CloseHandle(h: Handle) -> i32;
    fn QueryFullProcessImageNameW(h: Handle, flags: u32, path: *mut u16, size: *mut u32) -> i32;
}
pub mod focus {
    use super::*;
    #[derive(Clone, Debug, Default, serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct FocusTarget {
        pub app_name: Option<String>,
        pub bundle_id: Option<String>,
        #[serde(skip)]
        pub pid: Option<i32>,
    }
    impl FocusTarget {
        pub fn label(&self) -> &str {
            self.app_name.as_deref().unwrap_or("Unknown")
        }
        pub fn is_clide(&self) -> bool {
            self.pid == Some(std::process::id() as i32)
        }
    }
    pub fn frontmost() -> FocusTarget {
        unsafe {
            let mut pid = 0;
            GetWindowThreadProcessId(GetForegroundWindow(), &mut pid);
            let h = OpenProcess(0x1000, 0, pid);
            let mut path = vec![0u16; 32768];
            let mut len = path.len() as u32;
            let name = if !h.is_null()
                && QueryFullProcessImageNameW(h, 0, path.as_mut_ptr(), &mut len) != 0
            {
                let full = String::from_utf16_lossy(&path[..len as usize]);
                Some(
                    std::path::Path::new(&full)
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                )
            } else {
                None
            };
            if !h.is_null() {
                CloseHandle(h);
            }
            FocusTarget {
                app_name: name,
                bundle_id: None,
                pid: (pid != 0).then_some(pid as i32),
            }
        }
    }
}
pub use focus::FocusTarget;
pub mod ax {
    pub fn is_process_trusted() -> bool {
        true
    }
}
pub mod clipboard {
    use super::*;
    static CLIPBOARD: Mutex<()> = Mutex::new(());
    pub fn set_text(text: &str) -> bool {
        let _guard = CLIPBOARD.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            let deadline = Instant::now() + Duration::from_millis(150);
            while OpenClipboard(std::ptr::null_mut()) == 0 {
                if Instant::now() >= deadline {
                    return false;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            let units: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
            let mem = GlobalAlloc(2, units.len() * 2);
            let success = if mem.is_null() {
                false
            } else {
                let ptr = GlobalLock(mem);
                if ptr.is_null() {
                    GlobalFree(mem);
                    false
                } else {
                    std::ptr::copy_nonoverlapping(units.as_ptr(), ptr.cast::<u16>(), units.len());
                    GlobalUnlock(mem);
                    if EmptyClipboard() != 0 && !SetClipboardData(13, mem).is_null() {
                        true
                    } else {
                        GlobalFree(mem);
                        false
                    }
                }
            };
            CloseClipboard();
            success
        }
    }
    pub fn type_text_while_held(text: &str) -> Result<(), String> {
        super::type_unicode(text, None)
    }
}
fn event(vk: u16, scan: u16, flags: u32) -> Input {
    Input {
        kind: 1,
        payload: Payload {
            keyboard: std::mem::ManuallyDrop::new(Keyboard {
                vk,
                scan,
                flags,
                time: 0,
                extra: 0,
            }),
        },
    }
}
fn type_unicode(text: &str, target: Option<i32>) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_millis(600);
    while [0x10, 0x11, 0x12, 0x5b, 0x5c]
        .iter()
        .any(|&k| unsafe { GetAsyncKeyState(k) < 0 })
    {
        if Instant::now() >= deadline {
            return Err("Release the shortcut modifiers and paste the transcript.".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if target.is_some() && focus::frontmost().pid != target {
        return Err("Focus changed; the transcript is on the clipboard.".into());
    }
    let events: Vec<Input> = text
        .encode_utf16()
        .flat_map(|s| [event(0, s, 4), event(0, s, 6)])
        .collect();
    let sent = unsafe {
        SendInput(
            events.len() as u32,
            events.as_ptr(),
            std::mem::size_of::<Input>() as i32,
        )
    };
    if sent as usize == events.len() {
        Ok(())
    } else {
        Err("Windows blocked or interrupted insertion. The complete transcript is on the clipboard; elevated apps may reject input.".into())
    }
}
#[derive(Debug)]
pub struct InsertionFailure {
    pub message: String,
    pub on_clipboard: bool,
}
pub fn insert(text: &str, target: &FocusTarget) -> Result<InsertionMethod, InsertionFailure> {
    if text.is_empty() {
        return Err(InsertionFailure {
            message: "No text to insert".into(),
            on_clipboard: false,
        });
    }
    let copied = clipboard::set_text(text);
    if !copied {
        return Err(InsertionFailure {
            message: "The clipboard is unavailable; recover the transcript from history.".into(),
            on_clipboard: false,
        });
    }
    if target.pid.is_none() {
        return Err(InsertionFailure {
            message: "No focused application was identified; paste the transcript manually.".into(),
            on_clipboard: true,
        });
    }
    type_unicode(text, target.pid)
        .map(|_| InsertionMethod::Typed)
        .map_err(|message| InsertionFailure {
            message,
            on_clipboard: true,
        })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn input_matches_win64_abi() {
        assert_eq!(std::mem::size_of::<Input>(), 40);
        assert_eq!(std::mem::size_of::<Keyboard>(), 24);
    }
}

// Win64 INPUT layout is required by SendInput; enforce it during cross-compilation.
const _: [(); 40] = [(); std::mem::size_of::<Input>()];
