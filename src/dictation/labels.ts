import type { DictationState } from "@/lib/types";
import type { Tone } from "@/components/StatusDot";

/**
 * One place that turns dictation state into words and a colour, so the HUD,
 * the dashboard, and the menu bar never disagree about what is happening.
 */
export function stateLabel(state: DictationState): string {
  switch (state.kind) {
    case "idle":
      return "Ready";
    case "capturing":
      return "Listening";
    case "finalizingAudio":
    case "transcribing":
    case "processing":
      return "Processing";
    case "inserting":
      return "Inserting";
    case "complete":
      return "Done";
    case "captureFailed":
      return "Microphone problem";
    case "transcriptionFailed":
      return "Transcription failed";
    case "processingFailed":
      return "Processing failed";
    case "insertionFailed":
      return "Couldn't insert";
  }
}

export function stateTone(state: DictationState): Tone {
  switch (state.kind) {
    case "idle":
      return "idle";
    case "capturing":
      return "busy";
    case "finalizingAudio":
    case "transcribing":
    case "processing":
    case "inserting":
      return "busy";
    case "complete":
      return "ready";
    default:
      return "problem";
  }
}

/** The explanatory line shown under a failure. */
export function failureDetail(state: DictationState): string | null {
  switch (state.kind) {
    case "captureFailed":
    case "transcriptionFailed":
    case "processingFailed":
    case "insertionFailed":
      return state.message;
    default:
      return null;
  }
}
