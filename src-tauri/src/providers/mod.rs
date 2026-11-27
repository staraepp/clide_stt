//! Transcription backends behind one normalised adapter interface.

#[cfg(target_os = "macos")]
pub mod apple;
pub mod capture;
pub mod error;
pub mod local;
pub mod registry;
pub mod traits;

pub use error::ProviderError;
pub use registry::ProviderRegistry;
pub use traits::{
    AudioClip, Capabilities, ModelInfo, ProviderDescriptor, Transcription, TranscriptionProvider,
    TranscriptionRequest,
};
