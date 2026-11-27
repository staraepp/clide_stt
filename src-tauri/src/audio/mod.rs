//! Microphone capture and the lifetime of the audio it produces.

pub mod clip;
pub mod decode;
#[cfg(target_os = "macos")]
pub mod ducking;
#[cfg(target_os = "windows")]
#[path = "ducking_windows.rs"]
pub mod ducking;
pub mod error;
pub mod recorder;
pub mod resample;
pub mod speech;

pub use clip::{RecordedClip, RECOVERY_WINDOW};
pub use error::AudioError;
pub use recorder::{Recorder, SampleTap};
