//! The contract every text-refinement backend implements.
//!
//! # Why this is not `TranscriptionProvider`
//!
//! `blueprint.md` §7 is explicit: **the speech-to-text provider and the rewrite
//! provider are separate architectural concepts, and changing your STT model
//! must never implicitly change your rewrite model.** Reusing the transcription
//! trait here would collapse exactly that distinction.
//!
//! So this is its own small trait, its own registry, and its own setting. A
//! user can dictate with Groq and refine with Apple Intelligence, or dictate
//! locally and not refine at all.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RefineError {
    #[error("{engine} is not available on this Mac")]
    Unavailable { engine: &'static str },

    #[error("{engine} is not ready yet — the model is still downloading")]
    NotReady { engine: &'static str },

    #[error("{engine} declined the request: {detail}")]
    Declined {
        engine: &'static str,
        detail: String,
    },

    #[error("{engine} could not refine the text: {detail}")]
    Failed {
        engine: &'static str,
        detail: String,
    },
}

impl RefineError {
    /// Whether the transcript should simply be used as-is.
    ///
    /// Refinement is a *nicety*. Every failure here is recoverable by falling
    /// back to the words the user actually said, so none of them should ever
    /// surface as a failed dictation.
    pub fn is_recoverable(&self) -> bool {
        true
    }
}

/// How aggressively the transcript may be rewritten.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RefineStyle {
    /// Punctuation, casing, and obvious slips only. Wording is preserved.
    #[default]
    Tidy,
    /// Spoken phrasing becomes written prose. Meaning preserved, wording not.
    Written,
}

impl RefineStyle {
    /// The instruction handed to the model.
    ///
    /// Written deliberately narrowly. A dictation tool that answers questions
    /// in the transcript, or helpfully expands on what was said, is a bug —
    /// the user asked for their own words back, tidier.
    pub fn instruction(self) -> &'static str {
        match self {
            RefineStyle::Tidy => concat!(
                "Rewrite the following dictated text with correct punctuation, ",
                "capitalisation and spacing. Remove filler words and repeated ",
                "words caused by speech. Keep the original wording and meaning ",
                "exactly. Do not answer questions, do not add information, and ",
                "do not add commentary. Reply with only the corrected text."
            ),
            RefineStyle::Written => concat!(
                "Rewrite the following dictated text as clear written prose. ",
                "Keep every fact and the original meaning and intent. Do not ",
                "answer questions in the text, do not add information, and do ",
                "not add commentary. Reply with only the rewritten text."
            ),
        }
    }
}

#[derive(Clone, Debug)]
pub struct RefineRequest {
    pub text: String,
    pub style: RefineStyle,
}

/// What a refinement backend is, for the settings UI.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefinerDescriptor {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Runs on this Mac. Nothing is sent anywhere.
    pub local: bool,
    /// Usable right now — the framework exists, the model is downloaded, the
    /// user has the feature switched on at the OS level.
    pub available: bool,
    /// When unavailable, why. Shown so the user can act on it.
    pub unavailable_reason: Option<String>,
}

#[async_trait]
pub trait Refiner: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn local(&self) -> bool;

    /// Whether this backend can run right now, and why not if it cannot.
    ///
    /// Checked before every use rather than cached: Apple Intelligence can be
    /// switched off in System Settings while Clide is running.
    fn availability(&self) -> Result<(), RefineError>;

    async fn refine(&self, request: RefineRequest) -> Result<String, RefineError>;

    fn descriptor(&self) -> RefinerDescriptor {
        let availability = self.availability();
        RefinerDescriptor {
            id: self.id().to_string(),
            name: self.name().to_string(),
            description: self.description().to_string(),
            local: self.local(),
            available: availability.is_ok(),
            unavailable_reason: availability.err().map(|error| error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_styles_forbid_the_model_answering_the_text() {
        for style in [RefineStyle::Tidy, RefineStyle::Written] {
            let instruction = style.instruction().to_lowercase();
            assert!(
                instruction.contains("do not answer"),
                "{style:?} does not forbid answering the transcript"
            );
            assert!(
                instruction.contains("do not add"),
                "{style:?} does not forbid adding information"
            );
        }
    }

    #[test]
    fn tidy_preserves_wording_and_written_does_not_claim_to() {
        assert!(RefineStyle::Tidy
            .instruction()
            .contains("Keep the original wording"));
        assert!(RefineStyle::Written
            .instruction()
            .contains("Keep every fact"));
    }

    /// Refinement is a nicety layered on a transcript that already exists, so
    /// no failure here may ever lose the user's words.
    #[test]
    fn every_failure_is_recoverable() {
        let failures = [
            RefineError::Unavailable { engine: "test" },
            RefineError::NotReady { engine: "test" },
            RefineError::Declined {
                engine: "test",
                detail: "no".into(),
            },
            RefineError::Failed {
                engine: "test",
                detail: "no".into(),
            },
        ];
        for failure in failures {
            assert!(failure.is_recoverable(), "{failure} was not recoverable");
        }
    }

    #[test]
    fn the_default_style_is_the_conservative_one() {
        assert_eq!(RefineStyle::default(), RefineStyle::Tidy);
    }
}
