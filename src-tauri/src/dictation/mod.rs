//! Dictation: the state machine, the live transaction, and the pipeline that
//! moves audio through transcription, processing, and insertion.

pub mod events;
pub mod fallback;
#[cfg(target_os = "macos")]
pub mod live;
#[cfg(target_os = "windows")]
#[path = "live_windows.rs"]
pub mod live;
pub mod machine;
pub mod pipeline;
pub mod session;
pub mod text;

pub use machine::{DictationBehavior, DictationState};
pub use session::DictationSession;
