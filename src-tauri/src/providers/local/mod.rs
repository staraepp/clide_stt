//! Local transcription, running entirely on this Mac.
//!
//! No network, no credential, and the audio never leaves the machine — which is
//! why `Capabilities::local` exists rather than the pipeline special-casing it.
//!
//! Each engine offers only the models actually installed. An engine advertising
//! weights the user has not downloaded would fail at the worst possible moment,
//! so `models()` reads the disk.

mod audio;
mod parakeet;
mod whisper;

pub use parakeet::LocalParakeetProvider;
pub use whisper::LocalWhisperProvider;
