//! Text refinement through Apple Intelligence's on-device model.
//!
//! Uses the FoundationModels framework, which ships with macOS 26 on Apple
//! Silicon when the user has Apple Intelligence switched on. Nothing leaves the
//! machine, so this is the refinement backend Clide prefers.
//!
//! Availability is checked before every request rather than cached: Apple
//! Intelligence can be turned off in System Settings while Clide is running,
//! and a stale "yes" would turn a working dictation into a failed one.

use async_trait::async_trait;
use foundation_models::async_api::AsyncSession;
use foundation_models::{Availability, LanguageModelSession, SystemLanguageModel};

use super::traits::{RefineError, RefineRequest, Refiner};

const ENGINE_ID: &str = "apple-intelligence";

pub struct AppleIntelligenceRefiner;

impl AppleIntelligenceRefiner {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AppleIntelligenceRefiner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Refiner for AppleIntelligenceRefiner {
    fn id(&self) -> &'static str {
        ENGINE_ID
    }

    fn name(&self) -> &'static str {
        "Apple Intelligence"
    }

    fn description(&self) -> &'static str {
        "Apple's on-device model. Nothing leaves this Mac."
    }

    fn local(&self) -> bool {
        true
    }

    fn availability(&self) -> Result<(), RefineError> {
        match SystemLanguageModel::availability() {
            Availability::Available => Ok(()),
            // The inner reason distinguishes "not switched on" from "still
            // downloading", and the user can act on each differently.
            Availability::Unavailable(reason) => {
                let detail = format!("{reason:?}");
                if detail.to_lowercase().contains("notready")
                    || detail.to_lowercase().contains("downloading")
                {
                    Err(RefineError::NotReady { engine: ENGINE_ID })
                } else {
                    Err(RefineError::Unavailable { engine: ENGINE_ID })
                }
            }
            // The enum is non-exhaustive. An unrecognised state is treated as
            // unavailable rather than optimistically used.
            _ => Err(RefineError::Unavailable { engine: ENGINE_ID }),
        }
    }

    async fn refine(&self, request: RefineRequest) -> Result<String, RefineError> {
        self.availability()?;

        // The style's instruction is the session's system prompt, so the
        // transcript itself arrives as plain content. A transcript that reads
        // like a question is then far less likely to be answered rather than
        // tidied.
        let session = LanguageModelSession::with_instructions(request.style.instruction());
        let session = AsyncSession::new(&session);

        let response = session
            .respond(request.text.as_str())
            .map_err(|error| RefineError::Failed {
                engine: ENGINE_ID,
                detail: error.to_string(),
            })?
            .await
            .map_err(|error| RefineError::Failed {
                engine: ENGINE_ID,
                detail: error.to_string(),
            })?;

        let refined = response.content.trim().to_string();

        if refined.is_empty() {
            return Err(RefineError::Declined {
                engine: ENGINE_ID,
                detail: "the model returned nothing".into(),
            });
        }

        Ok(refined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_reports_itself_as_local() {
        let refiner = AppleIntelligenceRefiner::new();
        assert!(refiner.local());
        assert_eq!(refiner.id(), ENGINE_ID);
    }

    /// The descriptor must always render, whether or not Apple Intelligence is
    /// switched on — the settings screen needs to explain *why* it cannot be
    /// used, not omit the option.
    #[test]
    fn the_descriptor_explains_itself_when_unavailable() {
        let descriptor = AppleIntelligenceRefiner::new().descriptor();
        assert_eq!(descriptor.id, ENGINE_ID);
        assert!(descriptor.local);

        if descriptor.available {
            assert!(descriptor.unavailable_reason.is_none());
        } else {
            let reason = descriptor
                .unavailable_reason
                .expect("an unavailable engine must say why");
            assert!(!reason.is_empty());
        }
    }
}
