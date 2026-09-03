//! Local model management: what can be installed, what is, and fetching it.
//!
//! Local engines are first-class providers, not a special case bolted onto the
//! cloud path. This module owns the *weights*; `providers::local` owns turning
//! them into transcripts.

pub mod catalog;
pub mod hardware;
pub mod download;
pub mod rating;
pub mod store;

pub use catalog::{CatalogEntry, Engine, ModelFile};
pub use hardware::{hardware, Hardware};
pub use rating::{Fit, Rating};
pub use store::{ModelStatus, ModelStore};
