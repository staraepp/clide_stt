//! Local transcription with whisper.cpp.
//!
//! Runs whisper.cpp through `whisper-rs`, Metal-accelerated on Apple Silicon
//! and CUDA-accelerated in CUDA-enabled Windows builds. No network, no
//! credential, and the audio never leaves the machine — which is why
//! `Capabilities::local` exists rather than the pipeline special-casing it.
//!
//! The models this offers are exactly the ones actually installed. An engine
//! that advertises weights the user has not downloaded would fail at the worst
//! possible moment, so `models()` reads the disk.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_trait::async_trait;

use super::audio::read_wav_as_mono_f32;
use crate::models::{catalog, Engine, ModelStore};
use crate::providers::error::ProviderError;
use crate::providers::traits::{
    Capabilities, ModelInfo, Transcription, TranscriptionProvider, TranscriptionRequest,
};

const PROVIDER_ID: &str = "local-whisper";

/// The model currently held in memory, and which weights it came from.
///
/// Loading a Whisper model reads it from disk and compiles its GPU kernels,
/// which took ~10 seconds for a quantised large model. Doing that on every
/// dictation made a two-second job feel broken, so the loaded state is kept
/// and reused until a different model is chosen. The state holds the context
/// alive, so keeping it is enough.
type Loaded = Option<(PathBuf, whisper_rs::WhisperState)>;

pub struct LocalWhisperProvider {
    models: ModelStore,
    loaded: Arc<Mutex<Loaded>>,
}

impl LocalWhisperProvider {
    pub fn new(models: ModelStore) -> Self {
        Self {
            models,
            loaded: Arc::new(Mutex::new(None)),
        }
    }

    fn weights_for(&self, model_id: &str) -> Result<PathBuf, ProviderError> {
        let entry = catalog::find(model_id).ok_or_else(|| ProviderError::UnknownModel {
            provider: PROVIDER_ID,
            model: model_id.to_string(),
        })?;

        if !self.models.is_installed(&entry) {
            // Not "unknown" — the user picked a real model that simply is not
            // downloaded yet, and the message should say so.
            return Err(ProviderError::BadRequest {
                provider: PROVIDER_ID,
                detail: format!("{} is not downloaded yet", entry.name),
            });
        }

        Ok(self.models.path_for(&entry))
    }
}

#[async_trait]
impl TranscriptionProvider for LocalWhisperProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn name(&self) -> &'static str {
        "Local Whisper"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            local: true,
            batch: true,
            streaming: false,
            timestamps: true,
            word_timestamps: false,
            diarization: false,
            language_detection: true,
            translation: true,
            prompting: true,
        }
    }

    /// Only what is installed. The catalogue of *installable* models is a
    /// separate concept, served by `models::catalog` to the model manager UI.
    fn models(&self) -> Vec<ModelInfo> {
        self.models
            .installed()
            .into_iter()
            .filter(|status| status.entry.engine == Engine::Whisper)
            .map(|status| ModelInfo {
                id: status.entry.id,
                name: status.entry.name,
                description: status.entry.description,
                speed: status.entry.speed,
                quality: status.entry.quality,
                multilingual: status.entry.multilingual,
            })
            .collect()
    }

    fn default_model(&self) -> &'static str {
        #[cfg(target_os = "macos")]
        { "whisper-large-v3-turbo" }
        #[cfg(target_os = "windows")]
        { "whisper-base" }
    }

    fn warm_up(&self, model: &str) {
        let Ok(weights) = self.weights_for(model) else {
            return;
        };
        let mut guard = self.loaded.lock().unwrap_or_else(|e| e.into_inner());
        let resident = matches!(guard.as_ref(), Some((path, _)) if path == &weights);
        if let Err(error) = ensure_loaded(&mut guard, &weights) {
            tracing::warn!(%error, "could not preload the speech model");
            return;
        }
        drop(guard);
        if !resident {
            // Creating a context loads weights but leaves the first GPU graph
            // cold. Prime it on this background worker, before the user speaks.
            // The result is discarded; no tokens survive into a dictation.
            let started = Instant::now();
            if let Err(error) =
                run_whisper(&self.loaded, &weights, &vec![0.0; 16_000], Some("en"), None)
            {
                tracing::warn!(%error, "could not prime the speech model");
            } else {
                tracing::info!(
                    warm_ms = started.elapsed().as_millis() as u64,
                    "speech inference primed"
                );
            }
        }
    }

    fn unload(&self) {
        *self.loaded.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    fn start_capture(
        &self,
        options: crate::providers::capture::CaptureOptions,
    ) -> Option<crate::providers::capture::CaptureTranscription> {
        let weights = self.weights_for(&options.model).ok()?;
        let loaded = Arc::clone(&self.loaded);
        crate::providers::capture::CaptureTranscription::spawn(move |samples| {
            run_capture_phrase(
                &loaded,
                &weights,
                samples,
                options.language.as_deref(),
                options.prompt.as_deref(),
            )
        })
    }

    async fn transcribe(
        &self,
        request: TranscriptionRequest,
    ) -> Result<Transcription, ProviderError> {
        let weights = self.weights_for(&request.model)?;
        let audio = read_wav_as_mono_f32(request.audio.path())?;

        let started = Instant::now();
        let model = request.model.clone();
        let language = request.language.clone();
        let prompt = request.prompt.clone();

        // Inference is CPU/GPU-bound and blocking; it must not run on an async
        // worker or it would stall every other task in the runtime.
        let loaded = Arc::clone(&self.loaded);
        let text = tauri::async_runtime::spawn_blocking(move || {
            run_whisper(
                &loaded,
                &weights,
                &audio,
                language.as_deref(),
                prompt.as_deref(),
            )
        })
        .await
        .map_err(|_| ProviderError::ServiceUnavailable {
            provider: PROVIDER_ID,
            status: 500,
        })??;

        Ok(Transcription {
            text,
            provider: PROVIDER_ID.to_string(),
            model,
            language: request.language,
            latency_ms: started.elapsed().as_millis() as u64,
        })
    }
}

/// Encoder frames per second of audio. Whisper's encoder always looks at a
/// 30-second window (1500 frames); a short dictation padded out to 30 seconds
/// spends most of its time encoding silence.
const ENCODER_FRAMES_PER_SECOND: f32 = 50.0;
const FULL_WINDOW_FRAMES: i32 = 1500;
/// Headroom past the end of the speech, and a floor below which accuracy
/// suffers more than the speed-up is worth.
const ENCODER_MARGIN_FRAMES: i32 = 150;
const MIN_ENCODER_FRAMES: i32 = 500;

/// How much of the encoder window this clip needs, or `None` for all of it.
fn audio_context_for(samples: usize) -> Option<i32> {
    let seconds = samples as f32 / 16_000.0;
    let needed = (seconds * ENCODER_FRAMES_PER_SECOND).ceil() as i32 + ENCODER_MARGIN_FRAMES;
    // Whisper reads the context in blocks of eight frames.
    let rounded = (needed + 7) / 8 * 8;
    let context = rounded.max(MIN_ENCODER_FRAMES);
    (context < FULL_WINDOW_FRAMES).then_some(context)
}

/// Make `weights` the resident model, loading it if it is not already.
fn ensure_loaded(loaded: &mut Loaded, weights: &std::path::Path) -> Result<(), ProviderError> {
    // Flash Attention corrupts decoding with this build's shortened audio
    // context on M2 (repeated nonsense on the benchmark fixture). Keep the
    // verified path; a faster kernel is useless if it changes the words.
    ensure_loaded_with_attention(loaded, weights, false)
}

fn ensure_loaded_with_attention(
    loaded: &mut Loaded,
    weights: &std::path::Path,
    flash_attention: bool,
) -> Result<(), ProviderError> {
    use whisper_rs::{WhisperContext, WhisperContextParameters};

    if matches!(loaded.as_ref(), Some((path, _)) if path == weights) {
        return Ok(());
    }

    let failure = |detail: String| ProviderError::BadRequest {
        provider: PROVIDER_ID,
        detail,
    };

    // Drop the old model first so two large models are never resident at once
    // while the new one loads.
    *loaded = None;

    let started = Instant::now();
    let context = WhisperContext::new_with_params(
        weights.to_string_lossy().as_ref(),
        WhisperContextParameters {
            flash_attn: flash_attention,
            ..Default::default()
        },
    )
    .map_err(|e| failure(format!("the model could not be loaded: {e}")))?;

    let state = context
        .create_state()
        .map_err(|e| failure(format!("the model could not be started: {e}")))?;

    tracing::info!(
        load_ms = started.elapsed().as_millis() as u64,
        "speech model loaded"
    );
    *loaded = Some((weights.to_path_buf(), state));
    Ok(())
}

fn run_whisper(
    loaded: &Mutex<Loaded>,
    weights: &std::path::Path,
    audio: &[f32],
    language: Option<&str>,
    prompt: Option<&str>,
) -> Result<String, ProviderError> {
    use whisper_rs::{FullParams, SamplingStrategy};

    let failure = |detail: String| ProviderError::BadRequest {
        provider: PROVIDER_ID,
        detail,
    };

    // Held for the whole run: two dictations never overlap, and a second one
    // waiting for the first is correct, not a problem.
    let mut guard = loaded.lock().unwrap_or_else(|e| e.into_inner());
    ensure_loaded(&mut guard, weights)?;

    let Some((_, state)) = guard.as_mut() else {
        return Err(failure("the model could not be loaded".into()));
    };

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    // Each phrase/dictation is independent. Reusing the model must not carry
    // tokens from a previous dictation (or an API request) into this one.
    params.set_no_context(true);
    // Dictation wants the words that were said, not a creative reading.
    params.set_temperature(0.0);
    // No retries at higher temperatures. Whisper re-decodes a segment up to
    // five times when its confidence looks low, which short clips with a
    // vocabulary prompt trigger constantly — and a creative second attempt is
    // the opposite of what dictation wants.
    params.set_temperature_inc(0.0);
    if let Some(context) = audio_context_for(audio.len()) {
        params.set_audio_ctx(context);
    }
    // Whisper's own silence guards, as a second line behind the recorder's
    // speech check: don't emit blank tokens, and drop segments it believes
    // contain no speech.
    params.set_suppress_blank(true);
    params.set_no_speech_thold(0.6);
    params.set_print_progress(false);
    params.set_print_special(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    // `None` leaves whisper to detect the language itself.
    params.set_language(language);
    if let Some(prompt) = prompt {
        params.set_initial_prompt(prompt);
    }

    if let Err(error) = state.full(params, audio) {
        // A state that failed mid-run is not trusted again.
        *guard = None;
        return Err(failure(format!("transcription failed: {error}")));
    }

    let Some((_, state)) = guard.as_ref() else {
        return Err(failure("the model could not be loaded".into()));
    };

    // Segments carry borrowed UTF-8 that can be split mid-character on a
    // truncated decode, so read them lossily rather than dropping the segment.
    let mut text = String::new();
    for segment in state.as_iter() {
        if let Ok(chunk) = segment.to_str_lossy() {
            text.push_str(&chunk);
        }
    }

    Ok(text.trim().to_string())
}

fn run_capture_phrase(
    loaded: &Mutex<Loaded>,
    weights: &std::path::Path,
    samples: &[i16],
    language: Option<&str>,
    prompt: Option<&str>,
) -> Result<String, String> {
    let audio: Vec<f32> = samples.iter().map(|&s| s as f32 / 32768.0).collect();
    let started = Instant::now();
    let text = run_whisper(loaded, weights, &audio, language, prompt)
        .map_err(|error| error.to_string())?;
    // An ambiguous/empty phrase must not disappear from the dictation.
    let speech = crate::audio::speech::analyze(samples).speech;
    if text.trim().is_empty() || crate::audio::speech::is_phantom_transcript(&text, speech) {
        return Err("uncertain phrase; using the full recording".into());
    }
    tracing::info!(
        phrase_ms = started.elapsed().as_millis() as u64,
        audio_ms = samples.len() / 16,
        "capture phrase transcribed"
    );
    Ok(text)
}

/// Timing harness, not a test: `cargo test --release bench_whisper -- --ignored --nocapture`
/// with `CLIDE_BENCH_MODEL` (a ggml .bin) and `CLIDE_BENCH_WAV` (16 kHz mono).
/// Put a matching `-encoder.mlmodelc` next to the model to measure the Neural
/// Engine against the Metal GPU on the same clip.
#[cfg(test)]
mod bench {
    use super::*;

    /// Uses the actual Rust adapter and installed Metal backend. The capture
    /// case feeds PCM at microphone speed, then measures only the wait after
    /// release. No microphone permission or insertion into another app.
    #[tokio::test]
    #[ignore = "needs CLIDE_BENCH_MODEL and CLIDE_BENCH_WAV"]
    async fn bench_capture_latency() {
        use crate::providers::capture::CaptureTranscription;
        let model = PathBuf::from(std::env::var("CLIDE_BENCH_MODEL").unwrap());
        let wav = std::env::var("CLIDE_BENCH_WAV").unwrap();
        let audio = read_wav_as_mono_f32(std::path::Path::new(&wav)).unwrap();
        let prompt = format!(
            "{} {}",
            crate::context::KNOWN_NAMES,
            crate::context::TECHNICAL_VOCABULARY
        );
        for language in [None, Some("en")] {
            for attention in [false, true] {
                let loaded = Arc::new(Mutex::new(None));
                ensure_loaded_with_attention(&mut loaded.lock().unwrap(), &model, attention)
                    .unwrap();
                // Warm inference separately, as GPU graph initialization is
                // not a repeat-dictation cost.
                run_whisper(&loaded, &model, &audio, language, Some(&prompt)).unwrap();
                for run in 1..=2 {
                    let started = Instant::now();
                    let text =
                        run_whisper(&loaded, &model, &audio, language, Some(&prompt)).unwrap();
                    println!(
                        "BATCH language={language:?} flash={attention} run={run} ms={} text={text}",
                        started.elapsed().as_millis()
                    );
                }
                *loaded.lock().unwrap() = None;
            }
            let loaded = Arc::new(Mutex::new(None));
            ensure_loaded(&mut loaded.lock().unwrap(), &model).unwrap();
            let baseline = run_whisper(&loaded, &model, &audio, language, Some(&prompt)).unwrap();
            let engine = Arc::clone(&loaded);
            let weights = model.clone();
            let hint = prompt.clone();
            let run = CaptureTranscription::spawn(move |samples| {
                run_capture_phrase(&engine, &weights, samples, language, Some(&hint))
            })
            .unwrap();
            let tap = run.tap();
            let pcm: Vec<i16> = audio.iter().map(|&s| (s * 32767.0) as i16).collect();
            let playback = Instant::now();
            for (index, frame) in pcm.chunks(320).enumerate() {
                tap(frame);
                let due = std::time::Duration::from_millis((index as u64 + 1) * 20);
                tokio::time::sleep(due.saturating_sub(playback.elapsed())).await;
            }
            drop(tap);
            let released = Instant::now();
            let text = run.finish().await.unwrap();
            let release_ms = released.elapsed().as_millis();
            let words = |text: &str| {
                text.to_lowercase()
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|word| !word.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                words(&text),
                words(&baseline),
                "phrase boundaries changed the recognized words"
            );
            let corrected = crate::processing::backtrack::apply_backtracking(&text);
            let original_corrected = crate::processing::backtrack::apply_backtracking(&baseline);
            assert_eq!(
                words(&corrected.text),
                words(&original_corrected.text),
                "phrase boundaries changed a correction"
            );
            println!(
                "CAPTURE language={language:?} release_ms={} audio_ms={} text={text}",
                release_ms,
                pcm.len() / 16
            );
            *loaded.lock().unwrap() = None;
        }
    }

    #[test]
    #[ignore = "needs a model on disk"]
    fn bench_whisper() {
        let model = std::env::var("CLIDE_BENCH_MODEL").expect("CLIDE_BENCH_MODEL");
        let wav = std::env::var("CLIDE_BENCH_WAV").expect("CLIDE_BENCH_WAV");
        let audio =
            crate::providers::local::audio::read_wav_as_mono_f32(std::path::Path::new(&wav))
                .expect("wav");
        let loaded: Mutex<Loaded> = Mutex::new(None);
        // The prompt Clide really sends in a developer app, so the timings
        // include what the glossary costs.
        let prompt = std::env::var("CLIDE_BENCH_PROMPT").ok().map(|_| {
            format!(
                "{} {}",
                crate::context::KNOWN_NAMES,
                crate::context::TECHNICAL_VOCABULARY
            )
        });

        for run in 1..=4 {
            let started = Instant::now();
            let text = run_whisper(
                &loaded,
                std::path::Path::new(&model),
                &audio,
                Some("en"),
                prompt.as_deref(),
            )
            .expect("transcribe");
            println!(
                "run {run}: {:>6} ms  ({:.1}s of audio)  {}",
                started.elapsed().as_millis(),
                audio.len() as f32 / 16_000.0,
                text.trim()
            );
        }
        *loaded.lock().unwrap() = None;
    }
}

#[cfg(test)]
mod audio_context_tests {
    use super::audio_context_for;

    #[test]
    fn short_clips_use_a_small_window() {
        // Both sit under the floor, so both get the 10-second window.
        assert_eq!(audio_context_for(16_000), Some(500));
        assert_eq!(audio_context_for(5 * 16_000), Some(500));
    }

    #[test]
    fn the_window_grows_with_the_clip_in_blocks_of_eight() {
        let context = audio_context_for(15 * 16_000).unwrap();
        assert_eq!(context, 904);
        assert_eq!(context % 8, 0);
    }

    #[test]
    fn long_clips_use_the_whole_window() {
        assert_eq!(audio_context_for(28 * 16_000), None);
        assert_eq!(audio_context_for(120 * 16_000), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(name: &str) -> (LocalWhisperProvider, PathBuf) {
        let dir = std::env::temp_dir().join(format!("clide-local-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        (LocalWhisperProvider::new(ModelStore::new(&dir)), dir)
    }

    #[test]
    fn a_local_provider_declares_itself_local() {
        let (local, _dir) = provider("local");
        assert!(local.capabilities().local);
    }

    #[test]
    fn nothing_is_offered_until_something_is_installed() {
        let (local, _dir) = provider("empty");
        assert!(local.models().is_empty());
    }

    /// A model the user has not downloaded must produce a message about the
    /// download, not "unknown model" — it is a real model, just absent.
    #[tokio::test]
    async fn an_uninstalled_model_says_so() {
        let (local, _dir) = provider("uninstalled");
        let request = TranscriptionRequest {
            audio: crate::providers::traits::AudioClip::wav("/nonexistent.wav", 1.0),
            model: "whisper-base".into(),
            language: None,
            prompt: None,
        };
        let error = local.transcribe(request).await.unwrap_err();
        assert!(error.to_string().contains("not downloaded"), "got: {error}");
    }

    #[tokio::test]
    async fn a_model_outside_the_catalogue_is_unknown() {
        let (local, _dir) = provider("unknown");
        let request = TranscriptionRequest {
            audio: crate::providers::traits::AudioClip::wav("/nonexistent.wav", 1.0),
            model: "whisper-imaginary".into(),
            language: None,
            prompt: None,
        };
        let error = local.transcribe(request).await.unwrap_err();
        assert!(matches!(error, ProviderError::UnknownModel { .. }));
    }
}
