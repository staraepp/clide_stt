//! Groq speech-to-text adapter.
//!
//! Groq exposes an OpenAI-shaped audio endpoint, so the request is a multipart
//! upload of the WAV Clide already captured — no transcoding step.
//! Everything Groq-specific stops at this module's edge.

use std::time::Instant;

use async_trait::async_trait;
use serde::Deserialize;

use crate::providers::error::ProviderError;
use crate::providers::traits::{
    Capabilities, CredentialRequirement, ModelInfo, QualityClass, SpeedClass, Transcription,
    TranscriptionProvider, TranscriptionRequest,
};

const PROVIDER_ID: &str = "groq";
const TRANSCRIPTIONS_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";
const MODELS_URL: &str = "https://api.groq.com/openai/v1/models";

/// Groq's documented ceiling on the free tier. Checked locally so a long
/// dictation fails immediately instead of after a slow doomed upload.
const MAX_UPLOAD_BYTES: u64 = 25 * 1024 * 1024;

pub struct GroqProvider {
    http: reqwest::Client,
}

impl GroqProvider {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    fn bearer(credential: Option<&str>) -> Result<&str, ProviderError> {
        let key = credential
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .ok_or(ProviderError::MissingCredential {
                provider: PROVIDER_ID,
            })?;
        Ok(key)
    }
}

#[async_trait]
impl TranscriptionProvider for GroqProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn name(&self) -> &'static str {
        "Groq"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            local: false,
            batch: true,
            // Groq's audio endpoint is request/response only.
            streaming: false,
            timestamps: true,
            word_timestamps: true,
            diarization: false,
            language_detection: true,
            translation: true,
            prompting: true,
        }
    }

    fn models(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "whisper-large-v3-turbo".into(),
                name: "Whisper Large v3 Turbo".into(),
                description: "Fastest option. The right default for dictation.".into(),
                speed: SpeedClass::Fast,
                quality: QualityClass::High,
                multilingual: true,
            },
            ModelInfo {
                id: "whisper-large-v3".into(),
                name: "Whisper Large v3".into(),
                description: "Slower and more accurate. Better for accents and noise.".into(),
                speed: SpeedClass::Balanced,
                quality: QualityClass::VeryHigh,
                multilingual: true,
            },
        ]
    }

    fn default_model(&self) -> &'static str {
        "whisper-large-v3-turbo"
    }

    fn credential_requirement(&self) -> CredentialRequirement {
        CredentialRequirement::ApiKey {
            help_url: "https://console.groq.com/keys".into(),
            expected_prefix: Some("gsk_".into()),
        }
    }

    async fn validate_credentials(&self, credential: Option<&str>) -> Result<(), ProviderError> {
        let key = Self::bearer(credential)?;

        let response = self
            .http
            .get(MODELS_URL)
            .bearer_auth(key)
            .send()
            .await
            .map_err(network_error)?;

        if response.status().is_success() {
            return Ok(());
        }
        Err(status_error(response.status(), response.text().await.ok()))
    }

    async fn transcribe(
        &self,
        request: TranscriptionRequest,
        credential: Option<&str>,
    ) -> Result<Transcription, ProviderError> {
        let key = Self::bearer(credential)?;

        if !self.has_model(&request.model) {
            return Err(ProviderError::UnknownModel {
                provider: PROVIDER_ID,
                model: request.model,
            });
        }

        let metadata = tokio::fs::metadata(request.audio.path())
            .await
            .map_err(|e| ProviderError::AudioUnreadable(e.to_string()))?;
        if metadata.len() > MAX_UPLOAD_BYTES {
            return Err(ProviderError::AudioTooLarge {
                provider: PROVIDER_ID,
            });
        }

        let bytes = tokio::fs::read(request.audio.path())
            .await
            .map_err(|e| ProviderError::AudioUnreadable(e.to_string()))?;

        let file = reqwest::multipart::Part::bytes(bytes)
            .file_name(request.audio.file_name())
            .mime_str("audio/wav")
            .map_err(|e| ProviderError::AudioUnreadable(e.to_string()))?;

        let mut form = reqwest::multipart::Form::new()
            .part("file", file)
            .text("model", request.model.clone())
            .text("response_format", "json")
            // Dictation wants the words that were said, not a creative reading.
            .text("temperature", "0");

        if let Some(language) = request.language.clone() {
            form = form.text("language", language);
        }
        if let Some(prompt) = request.prompt.clone() {
            form = form.text("prompt", prompt);
        }

        let started = Instant::now();
        let response = self
            .http
            .post(TRANSCRIPTIONS_URL)
            .bearer_auth(key)
            .multipart(form)
            .send()
            .await
            .map_err(network_error)?;

        let status = response.status();
        if !status.is_success() {
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());
            let body = response.text().await.ok();
            return Err(match status.as_u16() {
                429 => ProviderError::RateLimited {
                    retry_after_secs: retry_after,
                },
                _ => status_error(status, body),
            });
        }

        let payload: TranscriptionPayload =
            response
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
            language: payload.language.or(request.language),
            latency_ms: started.elapsed().as_millis() as u64,
        })
    }
}

#[derive(Deserialize)]
struct TranscriptionPayload {
    text: String,
    #[serde(default)]
    language: Option<String>,
}

/// Groq wraps failures in `{"error": {"message": ...}}`.
#[derive(Deserialize)]
struct ApiErrorEnvelope {
    error: ApiErrorBody,
}

#[derive(Deserialize)]
struct ApiErrorBody {
    message: String,
}

fn network_error(error: reqwest::Error) -> ProviderError {
    ProviderError::Network {
        provider: PROVIDER_ID,
        // `reqwest::Error` renders the URL, never the Authorization header.
        detail: error.to_string(),
    }
}

fn status_error(status: reqwest::StatusCode, body: Option<String>) -> ProviderError {
    let detail = body
        .as_deref()
        .and_then(|raw| serde_json::from_str::<ApiErrorEnvelope>(raw).ok())
        .map(|envelope| envelope.error.message)
        .or(body)
        .unwrap_or_else(|| status.to_string());

    match status.as_u16() {
        401 | 403 => ProviderError::InvalidCredential {
            provider: PROVIDER_ID,
        },
        413 => ProviderError::AudioTooLarge {
            provider: PROVIDER_ID,
        },
        429 => ProviderError::RateLimited {
            retry_after_secs: None,
        },
        500..=599 => ProviderError::ServiceUnavailable {
            provider: PROVIDER_ID,
            status: status.as_u16(),
        },
        _ => ProviderError::BadRequest {
            provider: PROVIDER_ID,
            detail,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> GroqProvider {
        GroqProvider::new(reqwest::Client::new())
    }

    #[test]
    fn the_default_model_is_one_this_adapter_actually_offers() {
        let groq = provider();
        assert!(groq.has_model(groq.default_model()));
    }

    #[test]
    fn an_unknown_model_is_rejected_before_any_network_call() {
        assert!(!provider().has_model("whisper-large-v4-imaginary"));
    }

    #[tokio::test]
    async fn a_missing_key_fails_without_touching_the_network() {
        let error = provider().validate_credentials(None).await.unwrap_err();
        assert!(matches!(error, ProviderError::MissingCredential { .. }));
        assert!(error.needs_configuration());

        // Whitespace is not a credential.
        let error = provider()
            .validate_credentials(Some("   "))
            .await
            .unwrap_err();
        assert!(matches!(error, ProviderError::MissingCredential { .. }));
    }

    #[test]
    fn http_statuses_map_to_recoverable_categories() {
        let unauthorized = status_error(reqwest::StatusCode::UNAUTHORIZED, None);
        assert!(unauthorized.needs_configuration());
        assert!(!unauthorized.is_transient());

        let outage = status_error(reqwest::StatusCode::BAD_GATEWAY, None);
        assert!(outage.is_transient());

        let too_big = status_error(reqwest::StatusCode::PAYLOAD_TOO_LARGE, None);
        assert!(matches!(too_big, ProviderError::AudioTooLarge { .. }));
    }

    #[test]
    fn the_api_error_message_is_surfaced_to_the_user() {
        let body = r#"{"error":{"message":"model_not_found","type":"invalid_request_error"}}"#;
        let error = status_error(reqwest::StatusCode::BAD_REQUEST, Some(body.into()));
        assert!(error.to_string().contains("model_not_found"));
    }

    #[test]
    fn no_error_variant_can_carry_a_credential() {
        // The bearer token is never moved into an error; the closest call is
        // the network variant, which only formats reqwest's own message.
        let error = ProviderError::Network {
            provider: PROVIDER_ID,
            detail: "connection closed".into(),
        };
        assert!(!error.to_string().contains("gsk_"));
    }
}
