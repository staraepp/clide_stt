//! AssemblyAI speech-to-text adapter.
//!
//! Unlike the others, AssemblyAI is asynchronous: upload the audio, create a
//! transcript job, then poll until it settles. That costs a round trip or two
//! more than a single-request provider, so it suits imports better than live
//! dictation — which is a reason to *offer* it, not to hide it.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde::Deserialize;

use crate::providers::error::ProviderError;
use crate::providers::http;
use crate::providers::traits::{
    Capabilities, CredentialRequirement, ModelInfo, QualityClass, SpeedClass, Transcription,
    TranscriptionProvider, TranscriptionRequest,
};

const PROVIDER_ID: &str = "assemblyai";
const UPLOAD_URL: &str = "https://api.assemblyai.com/v2/upload";
const TRANSCRIPT_URL: &str = "https://api.assemblyai.com/v2/transcript";
const MAX_UPLOAD_BYTES: u64 = 512 * 1024 * 1024;

/// How often to ask whether the job is done, and when to give up. A dictation
/// clip settles in a few seconds; the ceiling exists so a stuck job surfaces as
/// a clear failure rather than hanging the pipeline forever.
const POLL_INTERVAL: Duration = Duration::from_millis(400);
const POLL_TIMEOUT: Duration = Duration::from_secs(120);

pub struct AssemblyAiProvider {
    http: reqwest::Client,
}

impl AssemblyAiProvider {
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

    async fn upload(&self, key: &str, bytes: Vec<u8>) -> Result<String, ProviderError> {
        let response = self
            .http
            .post(UPLOAD_URL)
            .header("authorization", key)
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(bytes)
            .send()
            .await
            .map_err(|e| http::network_error(PROVIDER_ID, e))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.ok();
            return Err(http::status_error(PROVIDER_ID, status, body, None));
        }

        let uploaded: Uploaded =
            response
                .json()
                .await
                .map_err(|e| ProviderError::MalformedResponse {
                    provider: PROVIDER_ID,
                    detail: e.to_string(),
                })?;
        Ok(uploaded.upload_url)
    }
}

#[async_trait]
impl TranscriptionProvider for AssemblyAiProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn name(&self) -> &'static str {
        "AssemblyAI"
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
        vec![
            ModelInfo {
                id: "best".into(),
                name: "Best".into(),
                description: "Highest accuracy. AssemblyAI picks the model.".into(),
                speed: SpeedClass::Balanced,
                quality: QualityClass::VeryHigh,
                multilingual: true,
            },
            ModelInfo {
                id: "nano".into(),
                name: "Nano".into(),
                description: "Cheaper and faster, across many languages.".into(),
                speed: SpeedClass::Fast,
                quality: QualityClass::Good,
                multilingual: true,
            },
        ]
    }

    fn default_model(&self) -> &'static str {
        "best"
    }

    fn credential_requirement(&self) -> CredentialRequirement {
        CredentialRequirement::ApiKey {
            help_url: "https://www.assemblyai.com/app/api-keys".into(),
            expected_prefix: None,
        }
    }

    async fn validate_credentials(&self, credential: Option<&str>) -> Result<(), ProviderError> {
        let key = Self::require_key(credential)?;

        // Listing transcripts is the cheapest authenticated read available.
        let response = self
            .http
            .get(TRANSCRIPT_URL)
            .query(&[("limit", "1")])
            .header("authorization", key)
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

        let started = Instant::now();
        let audio_url = self.upload(key, bytes).await?;

        let mut body = serde_json::json!({
            "audio_url": audio_url,
            "speech_model": request.model,
            "punctuate": true,
            "format_text": true,
        });
        match request.language.as_deref() {
            Some(language) => body["language_code"] = language.into(),
            None => body["language_detection"] = true.into(),
        }

        let created = self
            .http
            .post(TRANSCRIPT_URL)
            .header("authorization", key)
            .json(&body)
            .send()
            .await
            .map_err(|e| http::network_error(PROVIDER_ID, e))?;

        let status = created.status();
        if !status.is_success() {
            let text = created.text().await.ok();
            return Err(http::status_error(PROVIDER_ID, status, text, None));
        }

        let job: Job = created
            .json()
            .await
            .map_err(|e| ProviderError::MalformedResponse {
                provider: PROVIDER_ID,
                detail: e.to_string(),
            })?;

        let poll_url = format!("{TRANSCRIPT_URL}/{}", job.id);

        loop {
            if started.elapsed() > POLL_TIMEOUT {
                return Err(ProviderError::ServiceUnavailable {
                    provider: PROVIDER_ID,
                    status: 504,
                });
            }

            tokio::time::sleep(POLL_INTERVAL).await;

            let polled = self
                .http
                .get(&poll_url)
                .header("authorization", key)
                .send()
                .await
                .map_err(|e| http::network_error(PROVIDER_ID, e))?;

            let status = polled.status();
            if !status.is_success() {
                let text = polled.text().await.ok();
                return Err(http::status_error(PROVIDER_ID, status, text, None));
            }

            let job: Job = polled
                .json()
                .await
                .map_err(|e| ProviderError::MalformedResponse {
                    provider: PROVIDER_ID,
                    detail: e.to_string(),
                })?;

            match job.status.as_str() {
                "completed" => {
                    return Ok(Transcription {
                        text: job.text.unwrap_or_default(),
                        provider: PROVIDER_ID.to_string(),
                        model: request.model,
                        language: job.language_code.or(request.language),
                        latency_ms: started.elapsed().as_millis() as u64,
                    })
                }
                "error" => {
                    return Err(ProviderError::BadRequest {
                        provider: PROVIDER_ID,
                        detail: job
                            .error
                            .unwrap_or_else(|| "the transcription job failed".into()),
                    })
                }
                // "queued" / "processing" — keep waiting.
                _ => continue,
            }
        }
    }
}

#[derive(Deserialize)]
struct Uploaded {
    upload_url: String,
}

#[derive(Deserialize)]
struct Job {
    id: String,
    status: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    language_code: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> AssemblyAiProvider {
        AssemblyAiProvider::new(reqwest::Client::new())
    }

    #[test]
    fn the_default_model_is_one_this_adapter_offers() {
        let assembly = provider();
        assert!(assembly.has_model(assembly.default_model()));
    }

    #[tokio::test]
    async fn a_missing_key_fails_without_touching_the_network() {
        let error = provider().validate_credentials(None).await.unwrap_err();
        assert!(matches!(error, ProviderError::MissingCredential { .. }));
    }

    #[test]
    fn a_job_still_running_carries_no_text_and_must_not_be_treated_as_done() {
        let job: Job =
            serde_json::from_str(r#"{"id":"abc","status":"processing"}"#).unwrap();
        assert_eq!(job.status, "processing");
        assert!(job.text.is_none());
    }

    #[test]
    fn a_failed_job_carries_its_reason() {
        let job: Job =
            serde_json::from_str(r#"{"id":"abc","status":"error","error":"bad audio"}"#).unwrap();
        assert_eq!(job.error.as_deref(), Some("bad audio"));
    }
}
