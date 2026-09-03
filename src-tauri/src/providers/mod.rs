//! Transcription backends behind one normalised adapter interface.

pub mod assemblyai;
pub mod deepgram;
pub mod elevenlabs;
pub mod error;
pub mod groq;
pub mod http;
pub mod local;
pub mod openai;
pub mod openai_compatible;
pub mod registry;
pub mod traits;

pub use error::ProviderError;
pub use registry::ProviderRegistry;
pub use traits::{
    AudioClip, Capabilities, CredentialRequirement, ModelInfo, ProviderDescriptor, Transcription,
    TranscriptionProvider, TranscriptionRequest,
};
