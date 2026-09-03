//! ElevenLabs Scribe speech-to-text adapter.
//!
//! Multipart like the OpenAI shape, but the model field is `model_id`, auth is
//! the `xi-api-key` header rather than a bearer token, and the reply names the
//! language `language_code`.

use std::time::Instant;

use async_trait::async_trait;
use serde::Deserialize;

use crate::providers::error::ProviderError;
use crate::providers::http;
use crate::providers::traits::{
    Capabilities, CredentialRequirement, ModelInfo, QualityClass, SpeedClass, Transcription,
    TranscriptionProvider, TranscriptionRequest,
};

const PROVIDER_ID: &str = "elevenlabs";
const TRANSCRIPTIONS_URL: &str = "https://api.elevenlabs.io/v1/speech-to-text";
const USER_URL: &str = "https://api.elevenlabs.io/v1/user";
const MAX_UPLOAD_BYTES: u64 = 1024 * 1024 * 1024;

pub struct ElevenLabsProvider {
    http: reqwest::Client,
}

impl ElevenLabsProvider {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    fn require_key(credential: Option<&str>) -> Result<&str, ProviderError> {
        credential
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .ok_or(ProviderError::MissingCredential {
                provider: PROVIDER_ID,
            })
    }
}

#[async_trait]
impl TranscriptionProvider for ElevenLabsProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn name(&self) -> &'static str {
        "ElevenLabs"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            local: false,
            batch: true,
            streaming: false,
            timestamps: true,
            word_timestamps: true,
            diarization: true,
            language_detection: true,
            translation: false,
            prompting: false,
        }
    }

    fn models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "scribe_v1".into(),
            name: "Scribe v1".into(),
            description: "Accurate multilingual transcription with speaker labels.".into(),
            speed: SpeedClass::Balanced,
            quality: QualityClass::VeryHigh,
            multilingual: true,
        }]
    }

    fn default_model(&self) -> &'static str {
        "scribe_v1"
    }

    fn credential_requirement(&self) -> CredentialRequirement {
        CredentialRequirement::ApiKey {
            help_url: "https://elevenlabs.io/app/settings/api-keys".into(),
            expected_prefix: None,
        }
    }

    async fn validate_credentials(&self, credential: Option<&str>) -> Result<(), ProviderError> {
        let key = Self::require_key(credential)?;

        let response = self
            .http
            .get(USER_URL)
            .header("xi-api-key", key)
            .send()
            .await
            .map_err(|e| http::network_error(PROVIDER_ID, e))?;

        if response.status().is_success() {
            return Ok(());
        }
        let status = response.status();
        let body = response.text().await.ok();
        Err(http::status_error(PROVIDER_ID, status, body, None))
    }

    async fn transcribe(
        &self,
        request: TranscriptionRequest,
        credential: Option<&str>,
    ) -> Result<Transcription, ProviderError> {
        let key = Self::require_key(credential)?;

        if !self.has_model(&request.model) {
            return Err(ProviderError::UnknownModel {
                provider: PROVIDER_ID,
                model: request.model,
            });
        }

        let bytes = http::read_clip(&request.audio, PROVIDER_ID, MAX_UPLOAD_BYTES).await?;

        let file = reqwest::multipart::Part::bytes(bytes)
            .file_name(request.audio.file_name())
            .mime_str("audio/wav")
            .map_err(|e| ProviderError::AudioUnreadable(e.to_string()))?;

        let mut form = reqwest::multipart::Form::new()
            .part("file", file)
            .text("model_id", request.model.clone());

        if let Some(language) = request.language.clone() {
            form = form.text("language_code", language);
        }

        let started = Instant::now();
        let response = self
            .http
            .post(TRANSCRIPTIONS_URL)
            .header("xi-api-key", key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| http::network_error(PROVIDER_ID, e))?;

        let status = response.status();
        if !status.is_success() {
            let retry = http::retry_after(&response);
            let body = response.text().await.ok();
            return Err(http::status_error(PROVIDER_ID, status, body, retry));
        }

        let payload: Payload = response
            .json()
            .await
            .map_err(|e| ProviderError::MalformedResponse {
                provider: PROVIDER_ID,
                detail: e.to_string(),
            })?;

        Ok(Transcription {
            text: payload.text,
            provider: PROVIDER_ID.to_string(),
            model: request.model,
            language: payload.language_code.or(request.language),
            latency_ms: started.elapsed().as_millis() as u64,
        })
    }
}

#[derive(Deserialize)]
struct Payload {
    text: String,
    #[serde(default)]
    language_code: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> ElevenLabsProvider {
        ElevenLabsProvider::new(reqwest::Client::new())
    }

    #[test]
    fn the_default_model_is_one_this_adapter_offers() {
        let eleven = provider();
        assert!(eleven.has_model(eleven.default_model()));
    }

    #[tokio::test]
    async fn a_missing_key_fails_without_touching_the_network() {
        let error = provider().validate_credentials(None).await.unwrap_err();
        assert!(matches!(error, ProviderError::MissingCredential { .. }));
    }

    #[test]
    fn the_reply_language_field_is_read_under_its_own_name() {
        let payload: Payload =
            serde_json::from_str(r#"{"text":"hello","language_code":"en"}"#).unwrap();
        assert_eq!(payload.text, "hello");
        assert_eq!(payload.language_code.as_deref(), Some("en"));
    }
}
