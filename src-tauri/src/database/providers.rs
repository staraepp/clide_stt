//! Non-secret provider configuration: which model is selected, and whether a
//! credential has been stored for the provider. The credential itself is in
//! the Keychain and never reaches this table.

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub provider_id: String,
    pub model_id: Option<String>,
    /// Mirrors the Keychain; refreshed from it rather than trusted blindly.
    pub credential_configured: bool,
}

pub fn get(connection: &Connection, provider_id: &str) -> rusqlite::Result<Option<ProviderConfig>> {
    connection
        .query_row(
            "SELECT provider_id, model_id, credential_configured
             FROM provider_configs WHERE provider_id = ?1",
            [provider_id],
            |row| {
                Ok(ProviderConfig {
                    provider_id: row.get(0)?,
                    model_id: row.get(1)?,
                    credential_configured: row.get::<_, i64>(2)? != 0,
                })
            },
        )
        .optional()
}

pub fn set_model(
    connection: &Connection,
    provider_id: &str,
    model_id: &str,
    now: i64,
) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT INTO provider_configs (provider_id, model_id, credential_configured, updated_at)
         VALUES (?1, ?2, COALESCE((SELECT credential_configured FROM provider_configs WHERE provider_id = ?1), 0), ?3)
         ON CONFLICT(provider_id) DO UPDATE SET model_id = excluded.model_id, updated_at = excluded.updated_at",
        rusqlite::params![provider_id, model_id, now],
    )?;
    Ok(())
}

pub fn set_credential_configured(
    connection: &Connection,
    provider_id: &str,
    configured: bool,
    now: i64,
) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT INTO provider_configs (provider_id, model_id, credential_configured, updated_at)
         VALUES (?1, (SELECT model_id FROM provider_configs WHERE provider_id = ?1), ?2, ?3)
         ON CONFLICT(provider_id) DO UPDATE SET credential_configured = excluded.credential_configured, updated_at = excluded.updated_at",
        rusqlite::params![provider_id, configured as i64, now],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::schema;

    fn db() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        schema::apply(&connection).unwrap();
        connection
    }

    #[test]
    fn model_and_credential_flags_are_independent() {
        let connection = db();

        set_model(&connection, "groq", "whisper-large-v3-turbo", 1).unwrap();
        let config = get(&connection, "groq").unwrap().unwrap();
        assert_eq!(config.model_id.as_deref(), Some("whisper-large-v3-turbo"));
        assert!(!config.credential_configured);

        set_credential_configured(&connection, "groq", true, 2).unwrap();
        let config = get(&connection, "groq").unwrap().unwrap();
        // Storing a key must not reset the chosen model.
        assert_eq!(config.model_id.as_deref(), Some("whisper-large-v3-turbo"));
        assert!(config.credential_configured);

        set_model(&connection, "groq", "whisper-large-v3", 3).unwrap();
        let config = get(&connection, "groq").unwrap().unwrap();
        // Changing the model must not forget the key.
        assert!(config.credential_configured);
        assert_eq!(config.model_id.as_deref(), Some("whisper-large-v3"));
    }

    #[test]
    fn storing_a_credential_first_still_works() {
        let connection = db();
        set_credential_configured(&connection, "groq", true, 1).unwrap();
        let config = get(&connection, "groq").unwrap().unwrap();
        assert!(config.credential_configured);
        assert_eq!(config.model_id, None);
    }

    #[test]
    fn an_unconfigured_provider_has_no_row() {
        let connection = db();
        assert!(get(&connection, "groq").unwrap().is_none());
    }
}
