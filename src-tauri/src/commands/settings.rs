//! Settings and the aggregate readiness view the dashboard renders.

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::dictation::events;
use crate::dictation::machine::DictationBehavior;
use crate::permissions::{self, PermissionSnapshot};
use crate::processing::ProcessingMode;
use crate::settings::{AppSettings, VisualIntensity};
use crate::shortcuts;
use crate::state::AppState;

#[tauri::command]
pub fn get_settings(app: AppHandle) -> AppSettings {
    app.state::<AppState>().settings()
}

/// Everything the dashboard's System and Dictation cards need, in one call.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatus {
    pub permissions: PermissionSnapshot,
    pub settings: AppSettings,
    /// The accelerator macOS actually accepted, if any.
    pub registered_shortcut: Option<String>,
    pub shortcut_registered: bool,
    pub provider_name: String,
    pub model_name: String,
    pub provider_configured: bool,
    /// True when a dictation would work end to end right now.
    pub ready: bool,
}

#[tauri::command]
pub fn get_system_status(app: AppHandle) -> SystemStatus {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let permissions = permissions::snapshot();
    let registered_shortcut = state.registered_shortcut();

    let (provider_name, model_name) = match state.providers.get(&settings.provider_id) {
        Some(provider) => {
            let model_name = provider
                .models()
                .into_iter()
                .find(|model| model.id == settings.model_id)
                .map(|model| model.name)
                .unwrap_or_else(|| settings.model_id.clone());
            (provider.name().to_string(), model_name)
        }
        None => (settings.provider_id.clone(), settings.model_id.clone()),
    };

    let provider_configured = state.credentials.is_configured(&settings.provider_id);
    let shortcut_registered = registered_shortcut.is_some();

    SystemStatus {
        ready: permissions.can_capture()
            && permissions.can_insert()
            && shortcut_registered
            && provider_configured,
        permissions,
        settings,
        registered_shortcut,
        shortcut_registered,
        provider_name,
        model_name,
        provider_configured,
    }
}

/// Change the global shortcut.
///
/// Registration is attempted immediately so the user finds out here — not the
/// next time they try to dictate — that another app already owns the keys.
#[tauri::command]
pub fn set_shortcut(app: AppHandle, accelerator: String) -> Result<(), String> {
    if !shortcuts::is_valid(&accelerator) {
        return Err(format!("\"{accelerator}\" is not a valid shortcut."));
    }

    shortcuts::register(&app, &accelerator).map_err(|error| error.to_string())?;

    app.state::<AppState>()
        .update_settings(|settings| settings.shortcut = accelerator.clone())?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

#[tauri::command]
pub fn set_dictation_behavior(
    app: AppHandle,
    behavior: DictationBehavior,
) -> Result<(), String> {
    app.state::<AppState>()
        .update_settings(|settings| settings.behavior = behavior)?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

#[tauri::command]
pub fn set_processing_mode(app: AppHandle, mode: ProcessingMode) -> Result<(), String> {
    if !mode.is_available() {
        return Err("That mode is not available yet.".into());
    }
    app.state::<AppState>()
        .update_settings(|settings| settings.mode = mode)?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

#[tauri::command]
pub fn set_visual_intensity(app: AppHandle, intensity: VisualIntensity) -> Result<(), String> {
    app.state::<AppState>()
        .update_settings(|settings| settings.visual_intensity = intensity)?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

#[tauri::command]
pub fn set_language(app: AppHandle, language: Option<String>) -> Result<(), String> {
    let language = language.filter(|value| !value.trim().is_empty());
    app.state::<AppState>()
        .update_settings(|settings| settings.language = language.clone())?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

#[tauri::command]
pub fn complete_onboarding(app: AppHandle) -> Result<(), String> {
    app.state::<AppState>()
        .update_settings(|settings| settings.onboarding_complete = true)?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

/// Let the user go back through onboarding from settings.
#[tauri::command]
pub fn reset_onboarding(app: AppHandle) -> Result<(), String> {
    app.state::<AppState>()
        .update_settings(|settings| settings.onboarding_complete = false)?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

/// Choose what Clide may substitute when the selected engine cannot run.
///
/// `anyConfigured` is the only value that lets a recording reach a cloud vendor
/// the user did not pick, which is why it is opt-in rather than the default.
#[tauri::command]
pub fn set_fallback_policy(
    app: AppHandle,
    fallback: crate::dictation::fallback::FallbackPolicy,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.update_settings(|settings| settings.fallback = fallback)?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}

/// The refinement engines this build knows about, with live availability.
///
/// Availability is read fresh: Apple Intelligence can be switched off in
/// System Settings while Clide is running.
#[tauri::command]
pub fn list_refiners(app: AppHandle) -> Vec<crate::refine::RefinerDescriptor> {
    app.state::<AppState>().refiners.descriptors()
}

/// How far Rewrite may go. Only consulted in Rewrite mode.
#[tauri::command]
pub fn set_refine_style(
    app: AppHandle,
    style: crate::refine::RefineStyle,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.update_settings(|settings| settings.refine_style = style)?;
    events::emit_bare(&app, events::SETTINGS_CHANGED);
    Ok(())
}
