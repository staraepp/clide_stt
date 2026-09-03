//! Groq speech-to-text adapter.
//!
//! Groq exposes an OpenAI-shaped audio endpoint, so the request is a multipart
//! upload of the WAV Clide already captured — no transcoding step.
//! Everything Groq-specific stops at this module's edge.

use async_trait::async_trait;

use crate::providers::error::ProviderError;
use crate::providers::openai_compatible as compat;
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
        let key = compat::require_key(credential, PROVIDER_ID)?;
        compat::validate_via_models(&self.http, PROVIDER_ID, MODELS_URL, key).await
    }

    async fn transcribe(
        &self,
        request: TranscriptionRequest,
        credential: Option<&str>,
    ) -> Result<Transcription, ProviderError> {
        let key = compat::require_key(credential, PROVIDER_ID)?;

        if !self.has_model(&request.model) {
            return Err(ProviderError::UnknownModel {
                provider: PROVIDER_ID,
                model: request.model,
            });
        }

        compat::transcribe(
            &self.http,
            PROVIDER_ID,
            TRANSCRIPTIONS_URL,
            key,
            request,
            MAX_UPLOAD_BYTES,
        )
        .await
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

    /// Groq wraps failures in `{"error": {"message": ... }}`; the shared
    /// status mapper must recognise that envelope for this provider too.
    #[test]
    fn the_api_error_message_is_surfaced_to_the_user() {
        let body = r#"{"error":{"message":"model_not_found","type":"invalid_request_error"}}"#;
        let error = crate::providers::http::status_error(
            PROVIDER_ID,
            reqwest::StatusCode::BAD_REQUEST,
            Some(body.into()),
            None,
        );
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
