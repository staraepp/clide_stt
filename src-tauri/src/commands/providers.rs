//! Provider configuration.
//!
//! API keys enter through `save_provider_key` and are written straight to the
//! credential store. No command returns a key, and no command echoes one back
//! in an error message.

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::database::{now_ms, providers as provider_store};
use crate::providers::ProviderDescriptor;
use crate::state::AppState;

/// Everything the UI needs about a provider except the secret.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub id: String,
    pub name: String,
    /// Whether a credential is stored. Never the credential itself.
    pub configured: bool,
    pub model_id: String,
    pub model_name: String,
    pub selected: bool,
}

#[tauri::command]
pub fn list_providers(app: AppHandle) -> Vec<ProviderDescriptor> {
    app.state::<AppState>().providers.descriptors()
}

#[tauri::command]
pub fn get_provider_status(app: AppHandle) -> Result<Vec<ProviderStatus>, String> {
    let state = app.state::<AppState>();
    let settings = state.settings();

    Ok(state
        .providers
        .descriptors()
        .into_iter()
        .map(|descriptor| {
            let stored = provider_store::get(&state.db.lock(), &descriptor.id)
                .ok()
                .flatten();

            let model_id = stored
                .as_ref()
                .and_then(|config| config.model_id.clone())
                .unwrap_or_else(|| descriptor.default_model.clone());

            let model_name = descriptor
                .models
                .iter()
                .find(|model| model.id == model_id)
                .map(|model| model.name.clone())
                .unwrap_or_else(|| model_id.clone());

            ProviderStatus {
                configured: state.credentials.is_configured(&descriptor.id),
                selected: settings.provider_id == descriptor.id,
                id: descriptor.id,
                name: descriptor.name,
                model_id,
                model_name,
            }
        })
        .collect())
}

/// Store an API key.
///
/// The key is validated against the provider before being written, so a typo
/// is caught here rather than at the end of the user's first dictation.
#[tauri::command]
pub async fn save_provider_key(
    app: AppHandle,
    provider_id: String,
    key: String,
) -> Result<(), String> {
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err("Enter an API key first.".into());
    }

    let provider = app
        .state::<AppState>()
        .providers
        .get(&provider_id)
        .ok_or_else(|| format!("Unknown provider \"{provider_id}\"."))?;

    provider
        .validate_credentials(Some(&key))
        .await
        .map_err(|error| error.to_string())?;

    let state = app.state::<AppState>();
    state
        .credentials
        .store(&provider_id, &key)
        .map_err(|error| error.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn remove_provider_key(app: AppHandle, provider_id: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    state
        .credentials
        .delete(&provider_id)
        .map_err(|error| error.to_string())?;

    Ok(())
}

/// Check the stored credential against the provider.
#[tauri::command]
pub async fn validate_provider(app: AppHandle, provider_id: String) -> Result<(), String> {
    let provider = app
        .state::<AppState>()
        .providers
        .get(&provider_id)
        .ok_or_else(|| format!("Unknown provider \"{provider_id}\"."))?;

    let credential = app
        .state::<AppState>()
        .credentials
        .read(&provider_id)
        .map_err(|error| error.to_string())?;

    provider
        .validate_credentials(credential.as_deref())
        .await
        .map_err(|error| error.to_string())
}

/// Choose the provider and model used for dictation.
#[tauri::command]
pub fn select_provider(
    app: AppHandle,
    provider_id: String,
    model_id: Option<String>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let provider = state
        .providers
        .get(&provider_id)
        .ok_or_else(|| format!("Unknown provider \"{provider_id}\"."))?;

    let model_id = model_id.unwrap_or_else(|| provider.default_model().to_string());
    if !provider.has_model(&model_id) {
        return Err(format!(
            "{} does not offer the model \"{model_id}\".",
            provider.name()
        ));
    }

    provider_store::set_model(&state.db.lock(), &provider_id, &model_id, now_ms())
        .map_err(|error| error.to_string())?;

    state.update_settings(|settings| {
        settings.provider_id = provider_id.clone();
        settings.model_id = model_id.clone();
    })?;

    Ok(())
}
