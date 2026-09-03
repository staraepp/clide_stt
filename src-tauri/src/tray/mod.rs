//! The menu-bar item: status and access, nothing more.
//!
//! Deliberately boring. Provider and model configuration live in the app, not
//! in a menu that has to be re-read every time it grows.

use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Emitter, Manager};

use crate::dictation::{pipeline, DictationState};

const TRAY_ID: &str = "clide";

const OPEN: &str = "open";
const DICTATE: &str = "dictate";
const SETTINGS: &str = "settings";
const QUIT: &str = "quit";

pub fn build(app: &AppHandle) -> tauri::Result<TrayIcon> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, OPEN, "Open clide", true, None::<&str>)?,
            &MenuItem::with_id(app, DICTATE, "Start Dictation", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, SETTINGS, "Settings…", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, QUIT, "Quit clide", true, None::<&str>)?,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("clide — Ready")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(on_menu_event);

    if let Some(icon) = app.default_window_icon().cloned() {
        // Rendered as a template so macOS tints it for light and dark menu bars.
        builder = builder.icon(icon).icon_as_template(true);
    }

    builder.build(app)
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        OPEN => focus_main(app, None),
        SETTINGS => focus_main(app, Some("settings")),
        DICTATE => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move { pipeline::start(&app).await });
        }
        QUIT => app.exit(0),
        other => tracing::debug!(other, "unhandled tray menu item"),
    }
}

fn focus_main(app: &AppHandle, route: Option<&str>) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
    if let Some(route) = route {
        let _ = window.emit("navigate", route);
    }
}

/// Reflect the current dictation state in the menu-bar tooltip.
pub fn update_status(app: &AppHandle, state: &DictationState) {
    let label = match state {
        DictationState::Idle => "clide — Ready",
        DictationState::Capturing => "clide — Listening",
        DictationState::FinalizingAudio
        | DictationState::Transcribing { .. }
        | DictationState::Processing => "clide — Transcribing",
        DictationState::Inserting => "clide — Inserting",
        DictationState::Complete { .. } => "clide — Done",
        _ => "clide — Needs attention",
    };

    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(label));
    }
}
