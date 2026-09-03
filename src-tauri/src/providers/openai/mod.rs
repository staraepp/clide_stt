//! OpenAI speech-to-text adapter.
//!
//! Shares the wire format with Groq (see `openai_compatible`); what differs is
//! the model catalogue, the upload ceiling, and the key prefix.

use async_trait::async_trait;

use crate::providers::error::ProviderError;
use crate::providers::openai_compatible as compat;
use crate::providers::traits::{
    Capabilities, CredentialRequirement, ModelInfo, QualityClass, SpeedClass, Transcription,
    TranscriptionProvider, TranscriptionRequest,
};

const PROVIDER_ID: &str = "openai";
const TRANSCRIPTIONS_URL: &str = "https://api.openai.com/v1/audio/transcriptions";
const MODELS_URL: &str = "https://api.openai.com/v1/models";

/// OpenAI's documented ceiling for the audio endpoints.
const MAX_UPLOAD_BYTES: u64 = 25 * 1024 * 1024;

pub struct OpenAiProvider {
    http: reqwest::Client,
}

impl OpenAiProvider {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }
}

#[async_trait]
impl TranscriptionProvider for OpenAiProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn name(&self) -> &'static str {
        "OpenAI"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            local: false,
            batch: true,
            streaming: false,
            timestamps: true,
            // Only whisper-1 returns word timings; the gpt-4o models do not.
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
                id: "gpt-4o-mini-transcribe".into(),
                name: "GPT-4o mini Transcribe".into(),
                description: "Fast and inexpensive. A good default for dictation.".into(),
                speed: SpeedClass::Fast,
                quality: QualityClass::High,
                multilingual: true,
            },
            ModelInfo {
                id: "gpt-4o-transcribe".into(),
                name: "GPT-4o Transcribe".into(),
                description: "Most accurate. Better on accents and noisy rooms.".into(),
                speed: SpeedClass::Balanced,
                quality: QualityClass::VeryHigh,
                multilingual: true,
            },
            ModelInfo {
                id: "whisper-1".into(),
                name: "Whisper v2".into(),
                description: "The original Whisper endpoint. Returns word timings.".into(),
                speed: SpeedClass::Balanced,
                quality: QualityClass::High,
                multilingual: true,
            },
        ]
    }

    fn default_model(&self) -> &'static str {
        "gpt-4o-mini-transcribe"
    }

    fn credential_requirement(&self) -> CredentialRequirement {
        CredentialRequirement::ApiKey {
            help_url: "https://platform.openai.com/api-keys".into(),
            expected_prefix: Some("sk-".into()),
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

    fn provider() -> OpenAiProvider {
        OpenAiProvider::new(reqwest::Client::new())
    }

    #[test]
    fn the_default_model_is_one_this_adapter_offers() {
        let openai = provider();
        assert!(openai.has_model(openai.default_model()));
    }

    #[tokio::test]
    async fn a_missing_key_fails_without_touching_the_network() {
        for credential in [None, Some("  ")] {
            let error = provider()
                .validate_credentials(credential)
                .await
                .unwrap_err();
            assert!(matches!(error, ProviderError::MissingCredential { .. }));
        }
    }

    #[tokio::test]
    async fn an_unknown_model_is_rejected_before_the_upload() {
        let request = TranscriptionRequest {
            audio: crate::providers::traits::AudioClip::wav("/nonexistent.wav", 1.0),
            model: "whisper-large-v3".into(), // a Groq model, not an OpenAI one
            language: None,
            prompt: None,
        };
        let error = provider()
            .transcribe(request, Some("sk-test"))
            .await
            .unwrap_err();
        assert!(matches!(error, ProviderError::UnknownModel { .. }));
    }
}
