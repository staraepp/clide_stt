//! Non-secret provider configuration: which model is selected.
//!
//! Credential state deliberately does not live here. The credential store is
//! the single source of truth, so provider readiness cannot drift from a
//! mirrored SQLite flag.

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub provider_id: String,
    pub model_id: Option<String>,
}

pub fn get(connection: &Connection, provider_id: &str) -> rusqlite::Result<Option<ProviderConfig>> {
    connection
        .query_row(
            "SELECT provider_id, model_id
             FROM provider_configs WHERE provider_id = ?1",
            [provider_id],
            |row| {
                Ok(ProviderConfig {
                    provider_id: row.get(0)?,
                    model_id: row.get(1)?,
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
        "INSERT INTO provider_configs (provider_id, model_id, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(provider_id) DO UPDATE SET model_id = excluded.model_id, updated_at = excluded.updated_at",
        rusqlite::params![provider_id, model_id, now],
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
    fn model_round_trips_and_updates() {
        let connection = db();

        set_model(&connection, "groq", "whisper-large-v3-turbo", 1).unwrap();
        let config = get(&connection, "groq").unwrap().unwrap();
        assert_eq!(config.model_id.as_deref(), Some("whisper-large-v3-turbo"));
        assert_eq!(config.provider_id, "groq");

        set_model(&connection, "groq", "whisper-large-v3", 2).unwrap();
        let config = get(&connection, "groq").unwrap().unwrap();
        assert_eq!(config.model_id.as_deref(), Some("whisper-large-v3"));
    }

    #[test]
    fn an_unconfigured_provider_has_no_row() {
        let connection = db();
        assert!(get(&connection, "groq").unwrap().is_none());
    }
}
