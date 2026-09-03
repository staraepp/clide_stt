/**
 * Typed wrappers for every Tauri command.
 *
 * The UI never calls `invoke` directly: keeping the surface in one file means
 * the contract with Rust is reviewable in a single place, and a renamed
 * command breaks the build rather than a button.
 */
import { invoke } from "@tauri-apps/api/core";
import type {
  ModelsPage,
  AppSettings,
  DictationBehavior,
  DictationState,
  HistoryQuery,
  PermissionSnapshot,
  PermissionStatus,
  ProcessingMode,
  ProviderDescriptor,
  ProviderStatus,
  SystemStatus,
  Transcript,
  VisualIntensity,
} from "./types";

// --- dictation -------------------------------------------------------------

export const startDictation = () => invoke<void>("start_dictation");
export const stopDictation = () => invoke<void>("stop_dictation");
export const cancelDictation = () => invoke<void>("cancel_dictation");
export const retryDictation = () => invoke<void>("retry_dictation");
export const dismissDictation = () => invoke<void>("dismiss_dictation");
export const getDictationState = () =>
  invoke<DictationState>("get_dictation_state");

// --- permissions -----------------------------------------------------------

export const getPermissions = () =>
  invoke<PermissionSnapshot>("get_permissions");
export const requestMicrophonePermission = () =>
  invoke<PermissionStatus>("request_microphone_permission");
export const requestAccessibilityPermission = () =>
  invoke<PermissionStatus>("request_accessibility_permission");
export const openAccessibilitySettings = () =>
  invoke<void>("open_accessibility_settings");
export const openMicrophoneSettings = () =>
  invoke<void>("open_microphone_settings");

// --- providers -------------------------------------------------------------

export const listProviders = () =>
  invoke<ProviderDescriptor[]>("list_providers");
export const getProviderStatus = () =>
  invoke<ProviderStatus[]>("get_provider_status");

/** Validates the key with the provider, then stores it locally. */
export const saveProviderKey = (providerId: string, key: string) =>
  invoke<void>("save_provider_key", { providerId, key });

export const removeProviderKey = (providerId: string) =>
  invoke<void>("remove_provider_key", { providerId });

export const validateProvider = (providerId: string) =>
  invoke<void>("validate_provider", { providerId });

export const selectProvider = (providerId: string, modelId?: string) =>
  invoke<void>("select_provider", { providerId, modelId: modelId ?? null });

// --- history ---------------------------------------------------------------

export const getHistory = (query?: HistoryQuery) =>
  invoke<Transcript[]>("get_history", { query: query ?? null });

export const searchHistory = (search: string, limit?: number) =>
  invoke<Transcript[]>("search_history", { search, limit: limit ?? null });

export const deleteTranscript = (id: string) =>
  invoke<boolean>("delete_transcript", { id });

export const getSourceApps = () => invoke<string[]>("get_source_apps");

export const copyText = (text: string) => invoke<void>("copy_text", { text });

// --- settings --------------------------------------------------------------

export const getSettings = () => invoke<AppSettings>("get_settings");
export const getSystemStatus = () => invoke<SystemStatus>("get_system_status");

export const setShortcut = (accelerator: string) =>
  invoke<void>("set_shortcut", { accelerator });

export const setDictationBehavior = (behavior: DictationBehavior) =>
  invoke<void>("set_dictation_behavior", { behavior });

export const setProcessingMode = (mode: ProcessingMode) =>
  invoke<void>("set_processing_mode", { mode });

export const setVisualIntensity = (intensity: VisualIntensity) =>
  invoke<void>("set_visual_intensity", { intensity });

export const setLanguage = (language: string | null) =>
  invoke<void>("set_language", { language });

export const completeOnboarding = () => invoke<void>("complete_onboarding");
export const resetOnboarding = () => invoke<void>("reset_onboarding");

/** Tauri rejects with a plain string; normalise it for display. */
export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Something went wrong.";
}

/* --- Models --------------------------------------------------------------- */

/** Providers, models, and this Mac's hardware in one round trip. */
export const getModelsPage = () => invoke<ModelsPage>("get_models_page");

/**
 * Start a download. Returns as soon as it is under way — progress arrives as
 * `model:progress` and settles on `model:complete` or `model:failed`.
 */
export const downloadModel = (modelId: string) =>
  invoke<void>("download_model", { modelId });

export const removeModel = (modelId: string) =>
  invoke<void>("remove_model", { modelId });
