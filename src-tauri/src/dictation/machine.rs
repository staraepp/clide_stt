//! The dictation state machine.
//!
//! This module is deliberately pure: no Tauri, no audio, no network. Every
//! legal move a dictation transaction can make is expressed as
//! `transition(state, event)`, which makes the whole lifecycle testable
//! without a microphone or a provider.
//!
//! The UI renders this state directly rather than assembling its own booleans,
//! so `isRecording && isProcessing` can never both be true.

use serde::{Deserialize, Serialize};

/// Which stage of the transaction produced a failure. Failures stay
/// distinguishable because the recovery for each is different: a capture
/// failure has nothing to retry, a transcription failure has audio waiting,
/// and an insertion failure already holds a good transcript.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FailureStage {
    Capture,
    Transcription,
    Processing,
    Insertion,
}

/// How the transcript actually reached the focused application.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InsertionMethod {
    /// Written straight into the focused control via the Accessibility API.
    Accessibility,
    /// Typed in as Unicode keystrokes. Reaches apps that refuse both direct
    /// Accessibility writes and synthetic paste chords.
    Typed,
    /// Placed on the clipboard and pasted with a synthetic Cmd+V.
    ClipboardPaste,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DictationBehavior {
    /// Recording lives as long as the shortcut is held down.
    Hold,
    /// One press starts, the next press stops.
    Toggle,
}

/// The observable state of the current dictation transaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum DictationState {
    Idle,
    Capturing,
    FinalizingAudio,
    Transcribing {
        /// 1 for the first attempt; incremented by an explicit user retry.
        attempt: u32,
    },
    Processing,
    Inserting,
    Complete {
        transcript: String,
        method: InsertionMethod,
    },

    CaptureFailed {
        message: String,
    },
    TranscriptionFailed {
        message: String,
        /// Whether the audio is still around and a retry is worth offering.
        retryable: bool,
    },
    ProcessingFailed {
        message: String,
        /// The raw transcript survives a processing failure and stays offerable.
        transcript: String,
    },
    InsertionFailed {
        message: String,
        transcript: String,
        /// Whether we managed to leave the transcript on the clipboard.
        on_clipboard: bool,
    },
}

impl DictationState {
    /// True while Clide is holding the microphone open.
    pub fn is_capturing(&self) -> bool {
        matches!(self, Self::Capturing)
    }

    /// True while the transaction is mid-flight and cannot be restarted.
    pub fn is_busy(&self) -> bool {
        matches!(
            self,
            Self::Capturing
                | Self::FinalizingAudio
                | Self::Transcribing { .. }
                | Self::Processing
                | Self::Inserting
        )
    }

    /// True once the transaction has settled, successfully or not.
    pub fn is_settled(&self) -> bool {
        matches!(
            self,
            Self::Idle
                | Self::Complete { .. }
                | Self::CaptureFailed { .. }
                | Self::TranscriptionFailed { .. }
                | Self::ProcessingFailed { .. }
                | Self::InsertionFailed { .. }
        )
    }

    /// The transcript this state is holding, if any. Used so a successful
    /// transcription is never lost to a later failure.
    pub fn transcript(&self) -> Option<&str> {
        match self {
            Self::Complete { transcript, .. }
            | Self::ProcessingFailed { transcript, .. }
            | Self::InsertionFailed { transcript, .. } => Some(transcript),
            _ => None,
        }
    }

    /// Stable identifier used for logs and tests.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Capturing => "capturing",
            Self::FinalizingAudio => "finalizingAudio",
            Self::Transcribing { .. } => "transcribing",
            Self::Processing => "processing",
            Self::Inserting => "inserting",
            Self::Complete { .. } => "complete",
            Self::CaptureFailed { .. } => "captureFailed",
            Self::TranscriptionFailed { .. } => "transcriptionFailed",
            Self::ProcessingFailed { .. } => "processingFailed",
            Self::InsertionFailed { .. } => "insertionFailed",
        }
    }
}

/// Everything that can move a dictation transaction forward.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DictationInput {
    /// The shortcut fired (or the dashboard button was pressed).
    StartCapture,
    /// The shortcut was released, pressed again, or Stop was pressed.
    StopCapture,
    /// The recorder has flushed audio to disk.
    AudioFinalized,
    /// The provider returned a raw transcript.
    TranscriptReceived,
    /// The processing stage produced its final text.
    Processed,
    /// Text reached the focused application.
    Inserted {
        transcript: String,
        method: InsertionMethod,
    },
    /// The user asked to run the still-pending audio through the provider again.
    Retry,
    /// The user abandoned the transaction.
    Cancel,
    /// A settled state was acknowledged; return to Idle.
    Dismiss,
    Failed {
        stage: FailureStage,
        message: String,
        /// For transcription failures: is the audio still available?
        retryable: bool,
        /// For processing/insertion failures: the transcript worth keeping.
        transcript: Option<String>,
        /// For insertion failures: did the transcript land on the clipboard?
        on_clipboard: bool,
    },
}

impl DictationInput {
    pub fn failure(stage: FailureStage, message: impl Into<String>) -> Self {
        Self::Failed {
            stage,
            message: message.into(),
            retryable: false,
            transcript: None,
            on_clipboard: false,
        }
    }

    pub fn transcription_failure(message: impl Into<String>, retryable: bool) -> Self {
        Self::Failed {
            stage: FailureStage::Transcription,
            message: message.into(),
            retryable,
            transcript: None,
            on_clipboard: false,
        }
    }

    pub fn insertion_failure(
        message: impl Into<String>,
        transcript: impl Into<String>,
        on_clipboard: bool,
    ) -> Self {
        Self::Failed {
            stage: FailureStage::Insertion,
            message: message.into(),
            retryable: false,
            transcript: Some(transcript.into()),
            on_clipboard,
        }
    }
}

/// A move that the machine refuses to make. Callers treat this as "ignore the
/// input", not as a user-visible error: duplicate shortcut events and races
/// between the HUD and the tray both land here.
#[derive(Debug, PartialEq, Eq)]
pub struct IllegalTransition {
    pub from: &'static str,
    pub input: &'static str,
}

impl std::fmt::Display for IllegalTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot apply {} while {}", self.input, self.from)
    }
}

fn input_name(input: &DictationInput) -> &'static str {
    match input {
        DictationInput::StartCapture => "startCapture",
        DictationInput::StopCapture => "stopCapture",
        DictationInput::AudioFinalized => "audioFinalized",
        DictationInput::TranscriptReceived => "transcriptReceived",
        DictationInput::Processed => "processed",
        DictationInput::Inserted { .. } => "inserted",
        DictationInput::Retry => "retry",
        DictationInput::Cancel => "cancel",
        DictationInput::Dismiss => "dismiss",
        DictationInput::Failed { .. } => "failed",
    }
}

/// The single source of truth for legal dictation moves.
pub fn transition(
    state: &DictationState,
    input: DictationInput,
) -> Result<DictationState, IllegalTransition> {
    use DictationInput as I;
    use DictationState as S;

    let illegal = || IllegalTransition {
        from: state.name(),
        input: input_name(&input),
    };

    match (state, &input) {
        // --- happy path ---------------------------------------------------
        // A settled transaction can start a new one immediately; the user
        // should never have to dismiss a HUD before dictating again.
        (s, I::StartCapture) if s.is_settled() => Ok(S::Capturing),
        (S::Capturing, I::StopCapture) => Ok(S::FinalizingAudio),
        (S::FinalizingAudio, I::AudioFinalized) => Ok(S::Transcribing { attempt: 1 }),
        (S::Transcribing { .. }, I::TranscriptReceived) => Ok(S::Processing),
        (S::Processing, I::Processed) => Ok(S::Inserting),
        (S::Inserting, I::Inserted { transcript, method }) => Ok(S::Complete {
            transcript: transcript.clone(),
            method: *method,
        }),

        // --- explicit retry of still-pending audio ------------------------
        (S::TranscriptionFailed { retryable: true, .. }, I::Retry) => {
            Ok(S::Transcribing { attempt: 2 })
        }

        // --- abandonment ---------------------------------------------------
        (s, I::Cancel) if s.is_busy() => Ok(S::Idle),
        (s, I::Dismiss) if s.is_settled() => Ok(S::Idle),

        // --- failures -------------------------------------------------------
        (
            _,
            I::Failed {
                stage,
                message,
                retryable,
                transcript,
                on_clipboard,
            },
        ) => Ok(match stage {
            FailureStage::Capture => S::CaptureFailed {
                message: message.clone(),
            },
            FailureStage::Transcription => S::TranscriptionFailed {
                message: message.clone(),
                retryable: *retryable,
            },
            FailureStage::Processing => S::ProcessingFailed {
                message: message.clone(),
                transcript: transcript.clone().unwrap_or_default(),
            },
            FailureStage::Insertion => S::InsertionFailed {
                message: message.clone(),
                transcript: transcript.clone().unwrap_or_default(),
                on_clipboard: *on_clipboard,
            },
        }),

        _ => Err(illegal()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn advance(state: DictationState, input: DictationInput) -> DictationState {
        transition(&state, input).expect("legal transition")
    }

    /// The spinal cord, as a state walk.
    #[test]
    fn full_happy_path_reaches_complete() {
        let mut s = DictationState::Idle;
        s = advance(s, DictationInput::StartCapture);
        assert!(s.is_capturing());
        s = advance(s, DictationInput::StopCapture);
        assert_eq!(s, DictationState::FinalizingAudio);
        s = advance(s, DictationInput::AudioFinalized);
        assert_eq!(s, DictationState::Transcribing { attempt: 1 });
        s = advance(s, DictationInput::TranscriptReceived);
        assert_eq!(s, DictationState::Processing);
        s = advance(s, DictationInput::Processed);
        assert_eq!(s, DictationState::Inserting);
        s = advance(
            s,
            DictationInput::Inserted {
                transcript: "hello world".into(),
                method: InsertionMethod::Accessibility,
            },
        );
        assert_eq!(s.transcript(), Some("hello world"));
        assert!(s.is_settled());
    }

    #[test]
    fn a_settled_transaction_can_start_the_next_one() {
        for settled in [
            DictationState::Idle,
            DictationState::Complete {
                transcript: "x".into(),
                method: InsertionMethod::ClipboardPaste,
            },
            DictationState::CaptureFailed {
                message: "no device".into(),
            },
            DictationState::InsertionFailed {
                message: "no focus".into(),
                transcript: "x".into(),
                on_clipboard: true,
            },
        ] {
            assert_eq!(
                transition(&settled, DictationInput::StartCapture),
                Ok(DictationState::Capturing),
                "{} should allow a new capture",
                settled.name()
            );
        }
    }

    #[test]
    fn duplicate_shortcut_events_do_not_restart_capture() {
        let s = DictationState::Capturing;
        assert!(transition(&s, DictationInput::StartCapture).is_err());

        // A key-up arriving twice must not double-finalize the audio.
        let s = advance(s, DictationInput::StopCapture);
        assert!(transition(&s, DictationInput::StopCapture).is_err());
    }

    #[test]
    fn stages_cannot_be_skipped() {
        let s = DictationState::Capturing;
        assert!(transition(&s, DictationInput::TranscriptReceived).is_err());
        assert!(transition(&s, DictationInput::Processed).is_err());

        let s = DictationState::Transcribing { attempt: 1 };
        assert!(transition(&s, DictationInput::Processed).is_err());
    }

    #[test]
    fn each_failure_stage_stays_distinguishable() {
        let cases = [
            (FailureStage::Capture, "captureFailed"),
            (FailureStage::Transcription, "transcriptionFailed"),
            (FailureStage::Processing, "processingFailed"),
            (FailureStage::Insertion, "insertionFailed"),
        ];
        for (stage, expected) in cases {
            let s = transition(
                &DictationState::Transcribing { attempt: 1 },
                DictationInput::failure(stage, "boom"),
            )
            .unwrap();
            assert_eq!(s.name(), expected);
        }
    }

    /// A failed insertion is still a successful transcription.
    #[test]
    fn insertion_failure_preserves_the_transcript() {
        let s = transition(
            &DictationState::Inserting,
            DictationInput::insertion_failure("no editable field", "keep me", true),
        )
        .unwrap();

        assert_eq!(s.transcript(), Some("keep me"));
        assert!(matches!(
            s,
            DictationState::InsertionFailed {
                on_clipboard: true,
                ..
            }
        ));
    }

    #[test]
    fn retry_is_only_offered_while_audio_survives() {
        let retryable = DictationState::TranscriptionFailed {
            message: "503".into(),
            retryable: true,
        };
        assert_eq!(
            transition(&retryable, DictationInput::Retry),
            Ok(DictationState::Transcribing { attempt: 2 })
        );

        let expired = DictationState::TranscriptionFailed {
            message: "503".into(),
            retryable: false,
        };
        assert!(transition(&expired, DictationInput::Retry).is_err());
    }

    #[test]
    fn cancel_only_applies_to_in_flight_work() {
        assert_eq!(
            transition(&DictationState::Capturing, DictationInput::Cancel),
            Ok(DictationState::Idle)
        );
        assert_eq!(
            transition(&DictationState::Transcribing { attempt: 1 }, DictationInput::Cancel),
            Ok(DictationState::Idle)
        );
        // Nothing in flight to cancel.
        assert!(transition(&DictationState::Idle, DictationInput::Cancel).is_err());
    }

    #[test]
    fn busy_and_settled_never_overlap() {
        let states = [
            DictationState::Idle,
            DictationState::Capturing,
            DictationState::FinalizingAudio,
            DictationState::Transcribing { attempt: 1 },
            DictationState::Processing,
            DictationState::Inserting,
            DictationState::Complete {
                transcript: String::new(),
                method: InsertionMethod::Accessibility,
            },
            DictationState::CaptureFailed { message: String::new() },
            DictationState::TranscriptionFailed { message: String::new(), retryable: false },
            DictationState::ProcessingFailed { message: String::new(), transcript: String::new() },
            DictationState::InsertionFailed {
                message: String::new(),
                transcript: String::new(),
                on_clipboard: false,
            },
        ];
        for s in states {
            assert_ne!(s.is_busy(), s.is_settled(), "{} is both or neither", s.name());
        }
    }
}

#[cfg(test)]
mod wire_format {
    use super::*;

    /// The frontend renders this enum directly, so its serialised shape is a
    /// contract. This test fails loudly if a field is renamed or a variant
    /// changes tag.
    #[test]
    fn states_serialise_as_the_ui_expects() {
        let cases = [
            (DictationState::Idle, r#"{"kind":"idle"}"#),
            (DictationState::Capturing, r#"{"kind":"capturing"}"#),
            (
                DictationState::Transcribing { attempt: 2 },
                r#"{"kind":"transcribing","attempt":2}"#,
            ),
            (
                DictationState::Complete {
                    transcript: "hi".into(),
                    method: InsertionMethod::ClipboardPaste,
                },
                r#"{"kind":"complete","transcript":"hi","method":"clipboardPaste"}"#,
            ),
            (
                DictationState::InsertionFailed {
                    message: "nope".into(),
                    transcript: "hi".into(),
                    on_clipboard: true,
                },
                r#"{"kind":"insertionFailed","message":"nope","transcript":"hi","onClipboard":true}"#,
            ),
        ];

        for (state, expected) in cases {
            assert_eq!(serde_json::to_string(&state).unwrap(), expected);
        }
    }
}
