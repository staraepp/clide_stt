//! The floating recording HUD.
//!
//! It is a real window, but it must never behave like one: no focus, no dock
//! presence, no interception of clicks except when it is showing an error the
//! user has to act on.

use tauri::{AppHandle, Manager, PhysicalPosition, WebviewWindow};

use crate::state::AppState;

pub const LABEL: &str = "hud";

/// Gap between the HUD and the bottom of the screen, in logical pixels.
const BOTTOM_MARGIN: f64 = 96.0;

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

/// Show the HUD without taking focus from the app being dictated into.
///
/// Two things keep the caret where the user left it. `show()` is deliberately
/// never paired with `set_focus()`. And the window is declared `focusable:
/// false` in `tauri.conf.json`, which makes tao override `canBecomeKeyWindow`
/// on the underlying `NSWindow` — without that, macOS hands the HUD key status
/// as it appears and the transcript has nowhere to land.
pub fn show(app: &AppHandle) {
    let Some(window) = window(app) else {
        tracing::warn!("HUD window is missing");
        return;
    };

    position(app, &window);
    sync_interactivity(app, &window);

    if let Err(error) = window.show() {
        tracing::warn!(?error, "could not show the HUD");
    }
    let _ = window.set_always_on_top(true);
}

pub fn hide(app: &AppHandle) {
    if let Some(window) = window(app) {
        if let Err(error) = window.hide() {
            tracing::warn!(?error, "could not hide the HUD");
        }
    }
}

/// Keep the HUD click-through except when it is offering Retry or Copy.
///
/// A HUD that swallows clicks while someone is trying to work is worse than no
/// HUD, so interactivity is opt-in per state rather than always on.
pub fn sync_interactivity(app: &AppHandle, window: &WebviewWindow) {
    use crate::dictation::DictationState;

    // Only the failure states put controls (Retry, Copy) on screen.
    let needs_input = matches!(
        app.state::<AppState>().session.state(),
        DictationState::CaptureFailed { .. }
            | DictationState::TranscriptionFailed { .. }
            | DictationState::ProcessingFailed { .. }
            | DictationState::InsertionFailed { .. }
    );

    if let Err(error) = window.set_ignore_cursor_events(!needs_input) {
        tracing::debug!(?error, "could not update HUD cursor behaviour");
    }
}

/// Place the HUD at the bottom centre of whichever display the pointer is on,
/// so it appears next to the work the user is actually doing.
fn position(app: &AppHandle, window: &WebviewWindow) {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|point| app.monitor_from_point(point.x, point.y).ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());

    let Some(monitor) = monitor else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };

    let scale = monitor.scale_factor();
    let area = monitor.size();
    let origin = monitor.position();

    let x = origin.x + ((area.width as i32 - size.width as i32) / 2);
    let y = origin.y + area.height as i32
        - size.height as i32
        - (BOTTOM_MARGIN * scale).round() as i32;

    if let Err(error) = window.set_position(PhysicalPosition::new(x, y)) {
        tracing::debug!(?error, "could not position the HUD");
    }
}
