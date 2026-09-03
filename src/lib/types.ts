/**
 * The shapes Rust sends over the Tauri bridge.
 *
 * These mirror the `serde` output of the corresponding Rust types. The
 * dictation state in particular is a discriminated union rather than a bag of
 * booleans, so the UI cannot render a contradictory combination.
 */

export type DictationState =
  | { kind: "idle" }
  | { kind: "capturing" }
  | { kind: "finalizingAudio" }
  | { kind: "transcribing"; attempt: number }
  | { kind: "processing" }
  | { kind: "inserting" }
  | { kind: "complete"; transcript: string; method: InsertionMethod }
  | { kind: "captureFailed"; message: string }
  | { kind: "transcriptionFailed"; message: string; retryable: boolean }
  | { kind: "processingFailed"; message: string; transcript: string }
  | {
      kind: "insertionFailed";
      message: string;
      transcript: string;
      onClipboard: boolean;
    };

export type DictationStateKind = DictationState["kind"];

export type InsertionMethod = "accessibility" | "clipboardPaste";

export type DictationBehavior = "hold" | "toggle";

export type ProcessingMode = "verbatim" | "polished" | "rewrite";

export type VisualIntensity = "reduced" | "normal" | "high";

export type PermissionStatus =
  | "notDetermined"
  | "granted"
  | "denied"
  | "restricted";

export interface PermissionSnapshot {
  microphone: PermissionStatus;
  accessibility: PermissionStatus;
}

export interface AppSettings {
  shortcut: string;
  behavior: DictationBehavior;
  mode: ProcessingMode;
  providerId: string;
  modelId: string;
  language: string | null;
  visualIntensity: VisualIntensity;
  onboardingComplete: boolean;
}

export interface SystemStatus {
  permissions: PermissionSnapshot;
  settings: AppSettings;
  registeredShortcut: string | null;
  shortcutRegistered: boolean;
  providerName: string;
  modelName: string;
  providerConfigured: boolean;
  ready: boolean;
}

export interface Capabilities {
  local: boolean;
  batch: boolean;
  streaming: boolean;
  timestamps: boolean;
  wordTimestamps: boolean;
  diarization: boolean;
  languageDetection: boolean;
  translation: boolean;
  prompting: boolean;
}

export interface ModelInfo {
  id: string;
  name: string;
  description: string;
  speed: "fast" | "balanced" | "thorough";
  quality: "good" | "high" | "veryHigh";
  multilingual: boolean;
}

export type CredentialRequirement =
  | { kind: "none" }
  | { kind: "apiKey"; helpUrl: string; expectedPrefix: string | null };

export interface ProviderDescriptor {
  id: string;
  name: string;
  capabilities: Capabilities;
  models: ModelInfo[];
  defaultModel: string;
  credential: CredentialRequirement;
}

export interface ProviderStatus {
  id: string;
  name: string;
  /** Whether a credential is stored — never the credential itself. */
  configured: boolean;
  modelId: string;
  modelName: string;
  selected: boolean;
}

export type TranscriptSource = "dictation" | "import";

export interface Transcript {
  id: string;
  text: string;
  /** Unix milliseconds. */
  createdAt: number;
  source: TranscriptSource;
  sourceApp: string | null;
}

export interface HistoryQuery {
  search?: string;
  source?: TranscriptSource;
  sourceApp?: string;
  since?: number;
  until?: number;
  limit?: number;
  offset?: number;
}

/** True while the transaction is mid-flight and cannot be restarted. */
export function isBusy(state: DictationState): boolean {
  return (
    state.kind === "capturing" ||
    state.kind === "finalizingAudio" ||
    state.kind === "transcribing" ||
    state.kind === "processing" ||
    state.kind === "inserting"
  );
}

/** True once the transaction has settled, successfully or not. */
export function isFailure(state: DictationState): boolean {
  return state.kind.endsWith("Failed");
}

/** The transcript a state is holding, if any. */
export function transcriptOf(state: DictationState): string | null {
  switch (state.kind) {
    case "complete":
    case "processingFailed":
    case "insertionFailed":
      return state.transcript;
    default:
      return null;
  }
}
