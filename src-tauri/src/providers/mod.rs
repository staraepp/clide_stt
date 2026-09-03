//! Transcription backends behind one normalised adapter interface.

pub mod error;
pub mod groq;
pub mod registry;
pub mod traits;

pub use error::ProviderError;
pub use registry::ProviderRegistry;
pub use traits::{
    AudioClip, Capabilities, CredentialRequirement, ModelInfo, ProviderDescriptor, Transcription,
    TranscriptionProvider, TranscriptionRequest,
};
