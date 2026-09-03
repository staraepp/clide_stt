//! Application state shared across Tauri commands.

use std::sync::Mutex;

use crate::audio::Recorder;
use crate::credentials::Credentials;
use crate::database::Database;
use crate::models::ModelStore;
use crate::dictation::DictationSession;
use crate::providers::ProviderRegistry;
use crate::refine::RefinerRegistry;
use crate::settings::{self, AppSettings};

pub struct AppState {
    pub db: Database,
    /// Provider API keys. See `credentials` for why this is not the Keychain.
    pub credentials: Credentials,
    /// Local model weights on this machine.
    pub models: ModelStore,
    /// Shared HTTP client, reused for model downloads.
    pub http: reqwest::Client,
    pub recorder: Recorder,
    pub providers: ProviderRegistry,
    /// Text refinement, kept separate from transcription (blueprint §7).
    pub refiners: RefinerRegistry,
    pub session: DictationSession,

    /// Cached copy of the persisted preferences. The database stays the
    /// source of truth; this exists so the audio and shortcut paths never
    /// touch SQLite on a latency-sensitive code path.
    settings: Mutex<AppSettings>,

    /// The accelerator currently registered with macOS, if registration
    /// succeeded. `None` means the shortcut is configured but not active —
    /// usually because another app already owns it.
    registered_shortcut: Mutex<Option<String>>,
}

impl AppState {
    pub fn new(
        db: Database,
        credentials: Credentials,
        models: ModelStore,
        http: reqwest::Client,
        recorder: Recorder,
        providers: ProviderRegistry,
    ) -> Self {
        let settings = {
            let default_provider = providers.default_provider();
            let connection = db.lock();
            settings::load(
                &connection,
                default_provider.id(),
                default_provider.default_model(),
            )
        };

        Self {
            db,
            credentials,
            models,
            http,
            recorder,
            providers,
            refiners: RefinerRegistry::new(),
            session: DictationSession::new(),
            settings: Mutex::new(settings),
            registered_shortcut: Mutex::new(None),
        }
    }

    pub fn settings(&self) -> AppSettings {
        self.settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Mutate and persist preferences in one step, so the cache and the
    /// database cannot drift apart.
    pub fn update_settings(
        &self,
        edit: impl FnOnce(&mut AppSettings),
    ) -> Result<AppSettings, String> {
        let mut guard = self.settings.lock().unwrap_or_else(|e| e.into_inner());
        edit(&mut guard);
        settings::save(&self.db.lock(), &guard).map_err(|error| error.to_string())?;
        Ok(guard.clone())
    }

    pub fn registered_shortcut(&self) -> Option<String> {
        self.registered_shortcut
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn set_registered_shortcut(&self, accelerator: Option<String>) {
        *self
            .registered_shortcut
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = accelerator;
    }
}
