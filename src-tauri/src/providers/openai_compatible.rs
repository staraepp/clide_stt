//! The OpenAI audio-transcription wire format.
//!
//! Groq deliberately mirrors OpenAI's `/audio/transcriptions` endpoint, so both
//! adapters send the identical request. The shape lives here once; each adapter
//! keeps its own identity, models, limits, and credential rules.

use std::time::Instant;

use serde::Deserialize;

use super::error::ProviderError;
use super::http;
use super::traits::{Transcription, TranscriptionRequest};

#[derive(Deserialize)]
struct Payload {
    text: String,
    #[serde(default)]
    language: Option<String>,
}

/// POST the clip as multipart and normalise the reply.
///
/// `temperature` is pinned to zero: dictation wants the words that were said,
/// not a creative reading of them.
pub async fn transcribe(
    http_client: &reqwest::Client,
    provider: &'static str,
    url: &str,
    key: &str,
    request: TranscriptionRequest,
    max_bytes: u64,
) -> Result<Transcription, ProviderError> {
    let bytes = http::read_clip(&request.audio, provider, max_bytes).await?;

    let file = reqwest::multipart::Part::bytes(bytes)
        .file_name(request.audio.file_name())
        .mime_str("audio/wav")
        .map_err(|e| ProviderError::AudioUnreadable(e.to_string()))?;

    let mut form = reqwest::multipart::Form::new()
        .part("file", file)
        .text("model", request.model.clone())
        .text("response_format", "json")
        .text("temperature", "0");

    if let Some(language) = request.language.clone() {
        form = form.text("language", language);
    }
    if let Some(prompt) = request.prompt.clone() {
        form = form.text("prompt", prompt);
    }

    let started = Instant::now();
    let response = http_client
        .post(url)
        .bearer_auth(key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| http::network_error(provider, e))?;

    let status = response.status();
    if !status.is_success() {
        let retry = http::retry_after(&response);
        let body = response.text().await.ok();
        return Err(http::status_error(provider, status, body, retry));
    }

    let payload: Payload = response
        .json()
        .await
        .map_err(|e| ProviderError::MalformedResponse {
            provider,
            detail: e.to_string(),
        })?;

    Ok(Transcription {
        text: payload.text,
        provider: provider.to_string(),
        model: request.model,
        language: payload.language.or(request.language),
        latency_ms: started.elapsed().as_millis() as u64,
    })
}

/// Confirm a key by listing models — cheaper than spending a transcription.
pub async fn validate_via_models(
    http_client: &reqwest::Client,
    provider: &'static str,
    models_url: &str,
    key: &str,
) -> Result<(), ProviderError> {
    let response = http_client
        .get(models_url)
        .bearer_auth(key)
        .send()
        .await
        .map_err(|e| http::network_error(provider, e))?;

    if response.status().is_success() {
        return Ok(());
    }
    let status = response.status();
    let body = response.text().await.ok();
    Err(http::status_error(provider, status, body, None))
}

/// Reject an absent or whitespace-only credential before any network call.
pub fn require_key<'a>(
    credential: Option<&'a str>,
    provider: &'static str,
) -> Result<&'a str, ProviderError> {
    credential
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .ok_or(ProviderError::MissingCredential { provider })
}
