//! Provider credential storage.
//!
//! # Why this is not the Keychain
//!
//! Clide originally stored API keys in the macOS Keychain, which is what
//! `blueprint.md` §14 and `AGENTS.md` both call for. In practice that made the
//! app unusable during development: an ad-hoc-signed build gets a new code
//! identity every time it is rebuilt, macOS therefore sees a *different*
//! application asking for the same Keychain item, and the user is prompted to
//! authorise access again on every launch.
//!
//! The user asked for that to stop. This module is the deliberate consequence:
//! keys live in a file that only their account can read.
//!
//! ## What this costs
//!
//! The key is stored **in plaintext**, protected only by file permissions
//! (`0600`, inside a `0700` directory). Any process running as this user can
//! read it. The Keychain would have required user authorisation per app
//! identity; this does not.
//!
//! ## What is unchanged
//!
//! The key still never reaches SQLite, frontend state, logs, or any error
//! message. The UI still only ever learns `configured: true`. Nothing but
//! `transcribe` and `validate_credentials` sees the value.
//!
//! ## How to undo this
//!
//! Ship a build signed with a stable Developer ID. The identity then stops
//! changing between builds, the repeated prompts disappear, and this module can
//! go back to `security-framework`'s generic passwords with the same public
//! interface.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use thiserror::Error;

const FILE_NAME: &str = "credentials.json";

#[derive(Debug, Error)]
pub enum CredentialError {
    #[error("the credential store could not be read")]
    Unreadable,

    #[error("the credential store could not be written")]
    Unwritable,
}

/// Plaintext credential store, one entry per provider.
///
/// Cheap to clone: it is just a path. Reads and writes hit the disk each time
/// so that the file stays the single source of truth and cannot drift from an
/// in-memory cache.
#[derive(Clone, Debug)]
pub struct Credentials {
    path: PathBuf,
}

impl Credentials {
    /// `data_dir` is the app's data directory; the file is created lazily.
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join(FILE_NAME),
        }
    }

    pub fn store(&self, provider_id: &str, secret: &str) -> Result<(), CredentialError> {
        let mut all = self.read_all()?;
        all.insert(provider_id.to_string(), secret.to_string());
        self.write_all(&all)
    }

    /// Read a provider's API key. `Ok(None)` means "not configured".
    ///
    /// Callers must not log, serialise, or return this value to the frontend.
    pub fn read(&self, provider_id: &str) -> Result<Option<String>, CredentialError> {
        Ok(self.read_all()?.get(provider_id).cloned())
    }

    /// Remove a provider's key. Deleting something absent is a success.
    pub fn delete(&self, provider_id: &str) -> Result<(), CredentialError> {
        let mut all = self.read_all()?;
        if all.remove(provider_id).is_none() {
            return Ok(());
        }
        self.write_all(&all)
    }

    /// Whether a credential exists, without moving the secret anywhere.
    ///
    /// This is what the dashboard and provider cards use: the UI only ever
    /// learns `configured: true`.
    pub fn is_configured(&self, provider_id: &str) -> bool {
        matches!(self.read(provider_id), Ok(Some(_)))
    }

    fn read_all(&self) -> Result<BTreeMap<String, String>, CredentialError> {
        match std::fs::read_to_string(&self.path) {
            Ok(raw) => serde_json::from_str(&raw).map_err(|error| {
                // Never log the file's contents — it is entirely secrets.
                tracing::error!(?error, "the credential store is corrupt");
                CredentialError::Unreadable
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(BTreeMap::new())
            }
            Err(error) => {
                tracing::error!(?error, "could not read the credential store");
                Err(CredentialError::Unreadable)
            }
        }
    }

    fn write_all(&self, all: &BTreeMap<String, String>) -> Result<(), CredentialError> {
        let json = serde_json::to_string_pretty(all).map_err(|_| CredentialError::Unwritable)?;

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| CredentialError::Unwritable)?;
            restrict(parent, 0o700);
        }

        // Write, then tighten permissions before the value can be read by
        // anyone else. `create_dir_all` and `write` both respect umask, which
        // is not strict enough on its own.
        std::fs::write(&self.path, json).map_err(|error| {
            tracing::error!(?error, "could not write the credential store");
            CredentialError::Unwritable
        })?;
        restrict(&self.path, 0o600);

        Ok(())
    }
}

#[cfg(unix)]
fn restrict(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    if let Err(error) =
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
    {
        tracing::warn!(?error, ?path, "could not restrict permissions");
    }
}

#[cfg(not(unix))]
fn restrict(_path: &Path, _mode: u32) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(name: &str) -> (Credentials, PathBuf) {
        let dir = std::env::temp_dir().join(format!("clide-credentials-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        (Credentials::new(&dir), dir)
    }

    #[test]
    fn a_key_round_trips() {
        let (credentials, _dir) = store("round-trip");

        assert_eq!(credentials.read("groq").unwrap(), None);
        assert!(!credentials.is_configured("groq"));

        credentials.store("groq", "gsk_example").unwrap();
        assert_eq!(credentials.read("groq").unwrap().as_deref(), Some("gsk_example"));
        assert!(credentials.is_configured("groq"));
    }

    #[test]
    fn storing_again_replaces_rather_than_duplicates() {
        let (credentials, _dir) = store("replace");
        credentials.store("groq", "first").unwrap();
        credentials.store("groq", "second").unwrap();
        assert_eq!(credentials.read("groq").unwrap().as_deref(), Some("second"));
    }

    #[test]
    fn providers_do_not_share_a_slot() {
        let (credentials, _dir) = store("namespaced");
        credentials.store("groq", "a").unwrap();
        credentials.store("openai", "b").unwrap();
        assert_eq!(credentials.read("groq").unwrap().as_deref(), Some("a"));
        assert_eq!(credentials.read("openai").unwrap().as_deref(), Some("b"));
    }

    #[test]
    fn deleting_is_idempotent_and_leaves_others_alone() {
        let (credentials, _dir) = store("delete");
        credentials.store("groq", "a").unwrap();
        credentials.store("openai", "b").unwrap();

        credentials.delete("groq").unwrap();
        credentials.delete("groq").unwrap();

        assert!(!credentials.is_configured("groq"));
        assert!(credentials.is_configured("openai"));
    }

    /// The whole reason this file exists instead of the Keychain is that it is
    /// readable without a prompt — so it had better not be readable by anyone
    /// else either.
    #[cfg(unix)]
    #[test]
    fn the_file_is_not_readable_by_other_users() {
        use std::os::unix::fs::PermissionsExt;

        let (credentials, dir) = store("permissions");
        credentials.store("groq", "gsk_secret").unwrap();

        let mode = std::fs::metadata(dir.join(FILE_NAME))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "credential file is {mode:o}, expected 600");
    }

    #[test]
    fn a_corrupt_store_fails_loudly_rather_than_silently_losing_keys() {
        let (credentials, dir) = store("corrupt");
        std::fs::write(dir.join(FILE_NAME), "not json").unwrap();

        assert!(credentials.read("groq").is_err());
        // And `is_configured` degrades to false rather than panicking.
        assert!(!credentials.is_configured("groq"));
    }

    #[test]
    fn no_error_message_can_carry_a_secret() {
        for error in [CredentialError::Unreadable, CredentialError::Unwritable] {
            let rendered = error.to_string();
            assert!(!rendered.contains("gsk_"));
            assert!(!rendered.contains("sk-"));
        }
    }
}
