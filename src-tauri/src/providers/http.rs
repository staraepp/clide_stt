//! Shared HTTP plumbing for cloud transcription adapters.
//!
//! This is transport, not behaviour. Every adapter still decides its own
//! endpoint, auth scheme, request shape, and model list; what lives here is the
//! part that would otherwise be copied verbatim into each one — reading the
//! clip off disk, and turning a status code into a `ProviderError` the rest of
//! Clide can reason about.
//!
//! Nothing here inspects which provider it is working for beyond the `id` it is
//! handed for error messages.

use serde::Deserialize;

use super::error::ProviderError;
use super::traits::AudioClip;

/// Read a recording, refusing early if it exceeds the provider's ceiling.
///
/// Checked locally so a long dictation fails immediately rather than after a
/// slow upload that was always going to be rejected.
pub async fn read_clip(
    clip: &AudioClip,
    provider: &'static str,
    max_bytes: u64,
) -> Result<Vec<u8>, ProviderError> {
    let metadata = tokio::fs::metadata(clip.path())
        .await
        .map_err(|e| ProviderError::AudioUnreadable(e.to_string()))?;

    if metadata.len() > max_bytes {
        return Err(ProviderError::AudioTooLarge { provider });
    }

    tokio::fs::read(clip.path())
        .await
        .map_err(|e| ProviderError::AudioUnreadable(e.to_string()))
}

pub fn network_error(provider: &'static str, error: reqwest::Error) -> ProviderError {
    ProviderError::Network {
        provider,
        // `reqwest::Error` renders the URL, never the Authorization header.
        detail: error.to_string(),
    }
}

/// `Retry-After`, when the provider bothered to send one.
pub fn retry_after(response: &reqwest::Response) -> Option<u64> {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
}

/// Map a failed response onto the normalised error set.
///
/// `detail` is best-effort: providers disagree about where the human-readable
/// message lives, so several known envelope shapes are tried before falling
/// back to the raw body.
pub fn status_error(
    provider: &'static str,
    status: reqwest::StatusCode,
    body: Option<String>,
    retry_after_secs: Option<u64>,
) -> ProviderError {
    let detail = body
        .as_deref()
        .and_then(extract_message)
        .or(body)
        .unwrap_or_else(|| status.to_string());

    match status.as_u16() {
        401 | 403 => ProviderError::InvalidCredential { provider },
        413 => ProviderError::AudioTooLarge { provider },
        429 => ProviderError::RateLimited { retry_after_secs },
        500..=599 => ProviderError::ServiceUnavailable {
            provider,
            status: status.as_u16(),
        },
        _ => ProviderError::BadRequest { provider, detail },
    }
}

/// The error envelopes the supported providers actually use.
#[derive(Deserialize)]
#[serde(untagged)]
enum ErrorEnvelope {
    /// OpenAI, Groq: `{"error": {"message": "..."}}`
    Nested { error: NestedMessage },
    /// Deepgram: `{"err_msg": "..."}`
    DeepgramErrMsg { err_msg: String },
    /// ElevenLabs: `{"detail": {"message": "..."}}` or `{"detail": "..."}`
    Detail { detail: DetailBody },
    /// AssemblyAI: `{"error": "..."}`
    Flat { error: String },
    /// Several providers fall back to a bare `{"message": "..."}`.
    Message { message: String },
}

#[derive(Deserialize)]
struct NestedMessage {
    message: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum DetailBody {
    Structured { message: String },
    Plain(String),
}

fn extract_message(raw: &str) -> Option<String> {
    match serde_json::from_str::<ErrorEnvelope>(raw).ok()? {
        ErrorEnvelope::Nested { error } => Some(error.message),
        ErrorEnvelope::DeepgramErrMsg { err_msg } => Some(err_msg),
        ErrorEnvelope::Detail { detail } => Some(match detail {
            DetailBody::Structured { message } => message,
            DetailBody::Plain(text) => text,
        }),
        ErrorEnvelope::Flat { error } => Some(error),
        ErrorEnvelope::Message { message } => Some(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: &str = "test-provider";

    #[test]
    fn statuses_map_to_categories_a_user_can_act_on() {
        use reqwest::StatusCode;

        assert!(matches!(
            status_error(P, StatusCode::UNAUTHORIZED, None, None),
            ProviderError::InvalidCredential { .. }
        ));
        assert!(matches!(
            status_error(P, StatusCode::PAYLOAD_TOO_LARGE, None, None),
            ProviderError::AudioTooLarge { .. }
        ));
        assert!(matches!(
            status_error(P, StatusCode::TOO_MANY_REQUESTS, None, Some(30)),
            ProviderError::RateLimited {
                retry_after_secs: Some(30)
            }
        ));
        assert!(matches!(
            status_error(P, StatusCode::BAD_GATEWAY, None, None),
            ProviderError::ServiceUnavailable { .. }
        ));
        assert!(matches!(
            status_error(P, StatusCode::BAD_REQUEST, None, None),
            ProviderError::BadRequest { .. }
        ));
    }

    #[test]
    fn every_supported_error_envelope_yields_its_message() {
        let cases = [
            (r#"{"error":{"message":"bad audio"}}"#, "bad audio"),
            (r#"{"err_msg":"deepgram said no"}"#, "deepgram said no"),
            (r#"{"detail":{"message":"scribe said no"}}"#, "scribe said no"),
            (r#"{"detail":"plain detail"}"#, "plain detail"),
            (r#"{"error":"assembly said no"}"#, "assembly said no"),
            (r#"{"message":"bare message"}"#, "bare message"),
        ];

        for (raw, expected) in cases {
            assert_eq!(extract_message(raw).as_deref(), Some(expected), "{raw}");
        }
    }

    #[test]
    fn an_unrecognised_body_survives_as_its_own_detail() {
        let error = status_error(P, reqwest::StatusCode::BAD_REQUEST, Some("kaboom".into()), None);
        assert!(error.to_string().contains("kaboom"));
    }

    #[test]
    fn a_status_with_no_body_still_produces_a_usable_message() {
        let error = status_error(P, reqwest::StatusCode::BAD_REQUEST, None, None);
        assert!(!error.to_string().is_empty());
    }
}
