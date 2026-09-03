//! Non-secret application preferences.
//!
//! Stored as individual rows so that one unreadable value falls back to its
//! default instead of resetting everything the user configured.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::database::kv;
use crate::dictation::machine::DictationBehavior;
use crate::processing::ProcessingMode;

/// How much decorative rendering Clide is allowed to do.
///
/// This is a user preference, not a performance guess. macOS Reduce Motion is
/// honoured on top of it by the frontend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VisualIntensity {
    /// Static background. No ambient animation at all.
    Reduced,
    #[default]
    Normal,
    /// Ambient motion plus reaction to dictation state.
    High,
}

/// The default shortcut. Matches what clide.dev tells people to press, and
/// it is unclaimed by macOS itself.
pub const DEFAULT_SHORTCUT: &str = "Alt+Period";

mod keys {
    pub const SHORTCUT: &str = "shortcut";
    pub const BEHAVIOR: &str = "dictation.behavior";
    pub const MODE: &str = "processing.mode";
    pub const PROVIDER: &str = "provider.selected";
    pub const MODEL: &str = "provider.model";
    pub const LANGUAGE: &str = "dictation.language";
    pub const INTENSITY: &str = "visual.intensity";
    pub const ONBOARDING: &str = "onboarding.complete";
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// Tauri accelerator string, e.g. "Alt+Space".
    pub shortcut: String,
    pub behavior: DictationBehavior,
    pub mode: ProcessingMode,
    pub provider_id: String,
    pub model_id: String,
    /// ISO-639-1, or `None` to let the provider detect it.
    pub language: Option<String>,
    pub visual_intensity: VisualIntensity,
    pub onboarding_complete: bool,
}

impl AppSettings {
    /// Defaults for a machine that has never run Clide.
    pub fn defaults(provider_id: &str, model_id: &str) -> Self {
        Self {
            shortcut: DEFAULT_SHORTCUT.to_string(),
            behavior: DictationBehavior::Hold,
            mode: ProcessingMode::Polished,
            provider_id: provider_id.to_string(),
            model_id: model_id.to_string(),
            language: None,
            visual_intensity: VisualIntensity::Normal,
            onboarding_complete: false,
        }
    }
}

pub fn load(connection: &Connection, provider_id: &str, model_id: &str) -> AppSettings {
    let defaults = AppSettings::defaults(provider_id, model_id);

    AppSettings {
        shortcut: kv::get(connection, keys::SHORTCUT)
            .ok()
            .flatten()
            .unwrap_or(defaults.shortcut),
        behavior: kv::get(connection, keys::BEHAVIOR)
            .ok()
            .flatten()
            .unwrap_or(defaults.behavior),
        mode: kv::get(connection, keys::MODE)
            .ok()
            .flatten()
            .unwrap_or(defaults.mode),
        provider_id: kv::get(connection, keys::PROVIDER)
            .ok()
            .flatten()
            .unwrap_or(defaults.provider_id),
        model_id: kv::get(connection, keys::MODEL)
            .ok()
            .flatten()
            .unwrap_or(defaults.model_id),
        // `None` is persisted as JSON `null`, so deserialize the same
        // `Option<String>` shape that `save` writes. Reading it as a bare
        // `String` treats the valid null value as corrupt and warns at launch.
        language: kv::get::<Option<String>>(connection, keys::LANGUAGE)
            .ok()
            .flatten()
            .flatten(),
        visual_intensity: kv::get(connection, keys::INTENSITY)
            .ok()
            .flatten()
            .unwrap_or(defaults.visual_intensity),
        onboarding_complete: kv::get(connection, keys::ONBOARDING)
            .ok()
            .flatten()
            .unwrap_or(defaults.onboarding_complete),
    }
}

pub fn save(connection: &Connection, settings: &AppSettings) -> rusqlite::Result<()> {
    kv::set(connection, keys::SHORTCUT, &settings.shortcut)?;
    kv::set(connection, keys::BEHAVIOR, &settings.behavior)?;
    kv::set(connection, keys::MODE, &settings.mode)?;
    kv::set(connection, keys::PROVIDER, &settings.provider_id)?;
    kv::set(connection, keys::MODEL, &settings.model_id)?;
    kv::set(connection, keys::LANGUAGE, &settings.language)?;
    kv::set(connection, keys::INTENSITY, &settings.visual_intensity)?;
    kv::set(connection, keys::ONBOARDING, &settings.onboarding_complete)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;

    #[test]
    fn a_fresh_install_gets_working_defaults() {
        let db = Database::in_memory().unwrap();
        let settings = load(&db.lock(), "groq", "whisper-large-v3-turbo");

        assert_eq!(settings.shortcut, DEFAULT_SHORTCUT);
        assert_eq!(settings.behavior, DictationBehavior::Hold);
        assert!(!settings.onboarding_complete);
        // Rewrite must never be the default: it is not implemented.
        assert!(settings.mode.is_available());
    }

    #[test]
    fn settings_round_trip() {
        let db = Database::in_memory().unwrap();
        let mut settings = load(&db.lock(), "groq", "whisper-large-v3-turbo");
        settings.shortcut = "Ctrl+Shift+D".into();
        settings.behavior = DictationBehavior::Toggle;
        settings.mode = ProcessingMode::Verbatim;
        settings.visual_intensity = VisualIntensity::High;
        settings.language = Some("en".into());
        settings.onboarding_complete = true;
        save(&db.lock(), &settings).unwrap();

        let reloaded = load(&db.lock(), "groq", "whisper-large-v3-turbo");
        assert_eq!(reloaded.shortcut, "Ctrl+Shift+D");
        assert_eq!(reloaded.behavior, DictationBehavior::Toggle);
        assert_eq!(reloaded.mode, ProcessingMode::Verbatim);
        assert_eq!(reloaded.visual_intensity, VisualIntensity::High);
        assert_eq!(reloaded.language.as_deref(), Some("en"));
        assert!(reloaded.onboarding_complete);
    }

    #[test]
    fn automatic_language_round_trips_as_none() {
        let db = Database::in_memory().unwrap();
        let settings = load(&db.lock(), "groq", "whisper-large-v3-turbo");
        assert_eq!(settings.language, None);

        save(&db.lock(), &settings).unwrap();

        let reloaded = load(&db.lock(), "groq", "whisper-large-v3-turbo");
        assert_eq!(reloaded.language, None);
    }

    #[test]
    fn one_corrupt_value_does_not_reset_the_others() {
        let db = Database::in_memory().unwrap();
        let mut settings = load(&db.lock(), "groq", "whisper-large-v3-turbo");
        settings.shortcut = "Ctrl+Shift+D".into();
        save(&db.lock(), &settings).unwrap();

        db.lock()
            .execute(
                "UPDATE settings SET value = 'garbage' WHERE key = 'processing.mode'",
                [],
            )
            .unwrap();

        let reloaded = load(&db.lock(), "groq", "whisper-large-v3-turbo");
        assert_eq!(reloaded.shortcut, "Ctrl+Shift+D", "good values were lost");
        assert_eq!(
            reloaded.mode,
            ProcessingMode::Polished,
            "no fallback applied"
        );
    }

    #[test]
    fn no_setting_key_could_hold_a_credential() {
        // The settings table is explicitly not a place for secrets; this
        // asserts the key list stays that way.
        for key in [
            keys::SHORTCUT,
            keys::BEHAVIOR,
            keys::MODE,
            keys::PROVIDER,
            keys::MODEL,
            keys::LANGUAGE,
            keys::INTENSITY,
            keys::ONBOARDING,
        ] {
            assert!(!key.contains("key"), "{key} looks like a credential slot");
            assert!(!key.contains("secret"));
            assert!(!key.contains("token"));
        }
    }
}
