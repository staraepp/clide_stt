use thiserror::Error;

/// Failures normalised across every transcription backend.
///
/// Adapters translate their own wire errors into these so the rest of Clide
/// can reason about a failure without knowing which provider produced it.
/// No variant ever carries a credential.
#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("{provider} needs an API key before it can transcribe")]
    MissingCredential { provider: &'static str },

    #[error("{provider} rejected the API key")]
    InvalidCredential { provider: &'static str },

    #[error("the transcription provider is rate limiting requests")]
    RateLimited { retry_after_secs: Option<u64> },

    #[error("could not reach {provider}: {detail}")]
    Network {
        provider: &'static str,
        detail: String,
    },

    #[error("{provider} is temporarily unavailable ({status})")]
    ServiceUnavailable { provider: &'static str, status: u16 },

    #[error("{provider} rejected the request: {detail}")]
    BadRequest {
        provider: &'static str,
        detail: String,
    },

    #[error("the recording is too long for {provider}")]
    AudioTooLarge { provider: &'static str },

    #[error("{provider} does not offer the model \"{model}\"")]
    UnknownModel {
        provider: &'static str,
        model: String,
    },

    #[error("the recording could not be read: {0}")]
    AudioUnreadable(String),

    #[error("{provider} returned a response Clide could not read: {detail}")]
    MalformedResponse {
        provider: &'static str,
        detail: String,
    },
}

impl ProviderError {
    /// Whether the same request stands a chance of succeeding shortly.
    ///
    /// This shapes the wording of the failure, not Clide's behaviour: Clide
    /// never retries on its own and never silently moves audio to a different
    /// provider.
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::RateLimited { .. } | Self::Network { .. } | Self::ServiceUnavailable { .. }
        )
    }

    /// Whether the fix is for the user to go and configure something.
    pub fn needs_configuration(&self) -> bool {
        matches!(
            self,
            Self::MissingCredential { .. } | Self::InvalidCredential { .. }
        )
    }
}
