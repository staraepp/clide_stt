//! Deepgram speech-to-text adapter.
//!
//! Deepgram takes the audio as a raw request body rather than multipart, and
//! authenticates with `Authorization: Token <key>` rather than a bearer token.
//! Options travel as query parameters.

use std::time::Instant;

use async_trait::async_trait;
use serde::Deserialize;

use crate::providers::error::ProviderError;
use crate::providers::http;
use crate::providers::traits::{
    Capabilities, CredentialRequirement, ModelInfo, QualityClass, SpeedClass, Transcription,
    TranscriptionProvider, TranscriptionRequest,
};

const PROVIDER_ID: &str = "deepgram";
const LISTEN_URL: &str = "https://api.deepgram.com/v1/listen";
const PROJECTS_URL: &str = "https://api.deepgram.com/v1/projects";

/// Deepgram accepts far larger uploads than a dictation clip will ever be;
/// this is a sanity bound rather than a documented API limit.
const MAX_UPLOAD_BYTES: u64 = 100 * 1024 * 1024;

pub struct DeepgramProvider {
    http: reqwest::Client,
}

impl DeepgramProvider {
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
impl TranscriptionProvider for DeepgramProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn name(&self) -> &'static str {
        "Deepgram"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            local: false,
            batch: true,
            // Deepgram does stream, but Clide has no streaming path yet; this
            // claims only what this adapter actually implements.
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
        vec![
            ModelInfo {
                id: "nova-3".into(),
                name: "Nova 3".into(),
                description: "Deepgram's fastest and most accurate model.".into(),
                speed: SpeedClass::Fast,
                quality: QualityClass::VeryHigh,
                multilingual: true,
            },
            ModelInfo {
                id: "nova-2".into(),
                name: "Nova 2".into(),
                description: "Previous generation. Wider language coverage.".into(),
                speed: SpeedClass::Fast,
                quality: QualityClass::High,
                multilingual: true,
            },
        ]
    }

    fn default_model(&self) -> &'static str {
        "nova-3"
    }

    fn credential_requirement(&self) -> CredentialRequirement {
        CredentialRequirement::ApiKey {
            help_url: "https://console.deepgram.com/".into(),
            expected_prefix: None,
        }
    }

    async fn validate_credentials(&self, credential: Option<&str>) -> Result<(), ProviderError> {
        let key = Self::require_key(credential)?;

        let response = self
            .http
            .get(PROJECTS_URL)
            .header("Authorization", format!("Token {key}"))
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

        // `smart_format` supplies the punctuation and casing a dictated
        // sentence needs; Clide's own Polished pass handles the rest.
        let mut query: Vec<(&str, String)> = vec![
            ("model", request.model.clone()),
            ("smart_format", "true".into()),
        ];
        match request.language.as_deref() {
            Some(language) => query.push(("language", language.to_string())),
            None => query.push(("detect_language", "true".into())),
        }

        let started = Instant::now();
        let response = self
            .http
            .post(LISTEN_URL)
            .query(&query)
            .header("Authorization", format!("Token {key}"))
            .header(reqwest::header::CONTENT_TYPE, "audio/wav")
            .body(bytes)
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

        let channel = payload
            .results
            .and_then(|results| results.channels.into_iter().next())
            .ok_or(ProviderError::MalformedResponse {
                provider: PROVIDER_ID,
                detail: "the response carried no transcription channel".into(),
            })?;

        let alternative =
            channel
                .alternatives
                .into_iter()
                .next()
                .ok_or(ProviderError::MalformedResponse {
                    provider: PROVIDER_ID,
                    detail: "the response carried no transcript".into(),
                })?;

        Ok(Transcription {
            text: alternative.transcript,
            provider: PROVIDER_ID.to_string(),
            model: request.model,
            language: channel.detected_language.or(request.language),
            latency_ms: started.elapsed().as_millis() as u64,
        })
    }
}

#[derive(Deserialize)]
struct Payload {
    #[serde(default)]
    results: Option<Results>,
}

#[derive(Deserialize)]
struct Results {
    #[serde(default)]
    channels: Vec<Channel>,
}

#[derive(Deserialize)]
struct Channel {
    #[serde(default)]
    alternatives: Vec<Alternative>,
    #[serde(default)]
    detected_language: Option<String>,
}

#[derive(Deserialize)]
struct Alternative {
    #[serde(default)]
    transcript: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> DeepgramProvider {
        DeepgramProvider::new(reqwest::Client::new())
    }

    #[test]
    fn the_default_model_is_one_this_adapter_offers() {
        let deepgram = provider();
        assert!(deepgram.has_model(deepgram.default_model()));
    }

    #[tokio::test]
    async fn a_missing_key_fails_without_touching_the_network() {
        let error = provider().validate_credentials(Some(" ")).await.unwrap_err();
        assert!(matches!(error, ProviderError::MissingCredential { .. }));
    }

    #[test]
    fn a_transcript_is_read_out_of_the_nested_channel_shape() {
        let raw = r#"{"results":{"channels":[{"detected_language":"en",
            "alternatives":[{"transcript":"hello there"}]}]}}"#;
        let payload: Payload = serde_json::from_str(raw).unwrap();
        let channel = payload.results.unwrap().channels.pop_first();
        assert_eq!(channel.alternatives[0].transcript, "hello there");
        assert_eq!(channel.detected_language.as_deref(), Some("en"));
    }

    /// An empty result set must be a clean error, not a panic on `[0]`.
    #[test]
    fn an_empty_channel_list_does_not_panic() {
        let payload: Payload = serde_json::from_str(r#"{"results":{"channels":[]}}"#).unwrap();
        assert!(payload.results.unwrap().channels.is_empty());
    }

    trait PopFirst {
        fn pop_first(self) -> Channel;
    }
    impl PopFirst for Vec<Channel> {
        fn pop_first(mut self) -> Channel {
            self.remove(0)
        }
    }
}
