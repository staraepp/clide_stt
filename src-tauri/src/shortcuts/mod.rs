//! The one global shortcut.
//!
//! Clide registers a single accelerator and interprets it differently
//! depending on the user's chosen behaviour. Hold-to-talk needs both key
//! transitions, which is why the shortcut plugin's `Pressed`/`Released` states
//! are handled rather than a single "activated" callback.

use std::str::FromStr;

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::dictation::machine::DictationBehavior;
use crate::dictation::pipeline;
use crate::state::AppState;

#[derive(Debug, thiserror::Error)]
pub enum ShortcutError {
    #[error("\"{0}\" is not a valid shortcut")]
    Unparseable(String),

    #[error("another application is already using {0}")]
    Unavailable(String),
}

/// Register `accelerator`, replacing whatever was registered before.
///
/// On failure the previous shortcut is left unregistered and the app reports
/// the shortcut as inactive rather than pretending dictation is available.
pub fn register(app: &AppHandle, accelerator: &str) -> Result<(), ShortcutError> {
    let shortcut = Shortcut::from_str(accelerator)
        .map_err(|_| ShortcutError::Unparseable(accelerator.to_string()))?;

    unregister_current(app);

    app.global_shortcut()
        .register(shortcut)
        .map_err(|error| {
            tracing::warn!(accelerator, ?error, "shortcut registration failed");
            ShortcutError::Unavailable(accelerator.to_string())
        })?;

    app.state::<AppState>()
        .set_registered_shortcut(Some(accelerator.to_string()));
    tracing::info!(accelerator, "global shortcut registered");
    Ok(())
}

pub fn unregister_current(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Some(previous) = state.registered_shortcut() {
        if let Ok(shortcut) = Shortcut::from_str(&previous) {
            let _ = app.global_shortcut().unregister(shortcut);
        }
    }
    state.set_registered_shortcut(None);
}

/// Translate a key transition into a dictation command.
///
/// Duplicate or out-of-order events are safe: the state machine rejects moves
/// that do not apply, so a stray key-up cannot finalise audio twice.
pub fn on_shortcut(app: &AppHandle, state: ShortcutState) {
    let behavior = app.state::<AppState>().settings().behavior;
    let capturing = app.state::<AppState>().session.state().is_capturing();

    let action = match (behavior, state) {
        (DictationBehavior::Hold, ShortcutState::Pressed) => Action::Start,
        (DictationBehavior::Hold, ShortcutState::Released) => Action::Stop,
        (DictationBehavior::Toggle, ShortcutState::Pressed) => {
            if capturing {
                Action::Stop
            } else {
                Action::Start
            }
        }
        // In toggle mode the key coming back up means nothing.
        (DictationBehavior::Toggle, ShortcutState::Released) => return,
    };

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match action {
            Action::Start => pipeline::start(&app).await,
            Action::Stop => pipeline::stop(&app).await,
        }
    });
}

enum Action {
    Start,
    Stop,
}

/// Whether an accelerator string is well formed, without registering it.
/// Used by the shortcut recorder in settings and onboarding.
pub fn is_valid(accelerator: &str) -> bool {
    Shortcut::from_str(accelerator).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realistic_accelerators_parse() {
        for accelerator in [
            crate::settings::DEFAULT_SHORTCUT,
            "Ctrl+Space",
            "Alt+Shift+D",
            "CmdOrCtrl+Shift+Space",
            "F5",
        ] {
            assert!(is_valid(accelerator), "{accelerator} should be valid");
        }
    }

    #[test]
    fn nonsense_accelerators_are_rejected_before_registration() {
        for accelerator in ["", "NotAKey", "Alt+", "++"] {
            assert!(!is_valid(accelerator), "{accelerator} should be invalid");
        }
    }

    #[test]
    fn the_default_shortcut_is_registrable_in_principle() {
        assert!(is_valid(crate::settings::DEFAULT_SHORTCUT));
    }
}
