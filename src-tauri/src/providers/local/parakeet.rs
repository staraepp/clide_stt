//! Local transcription with NVIDIA Parakeet, through ONNX Runtime.
//!
//! Parakeet is a transducer rather than an encoder-decoder, and it ships as
//! four artifacts loaded from a directory rather than one weights file. Both
//! differences stop at this module's edge.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_trait::async_trait;

use super::audio::read_wav_as_mono_f32;
use crate::models::catalog::ParakeetArch;
use crate::models::{catalog, Engine, ModelStore};
use crate::providers::error::ProviderError;
use crate::providers::traits::{
    Capabilities, ModelInfo, Transcription, TranscriptionProvider, TranscriptionRequest,
};

const PROVIDER_ID: &str = "local-parakeet";

pub struct LocalParakeetProvider {
    models: ModelStore,
    loaded: Arc<Mutex<Option<Loaded>>>,
}

struct Loaded {
    directory: PathBuf,
    arch: ParakeetArch,
    model: ResidentModel,
}

enum ResidentModel {
    Tdt(parakeet_rs::ParakeetTDT),
    Ctc(parakeet_rs::Parakeet),
}

impl LocalParakeetProvider {
    pub fn new(models: ModelStore) -> Self {
        Self {
            models,
            loaded: Arc::new(Mutex::new(None)),
        }
    }

    /// Parakeet loads from a *directory*, not a file, and the two
    /// architectures take different code paths — so the entry is resolved
    /// alongside the path.
    fn resolve(&self, model_id: &str) -> Result<(PathBuf, ParakeetArch), ProviderError> {
        let entry = catalog::find(model_id).ok_or_else(|| ProviderError::UnknownModel {
            provider: PROVIDER_ID,
            model: model_id.to_string(),
        })?;

        if !self.models.is_installed(&entry) {
            // Not "unknown" — a real model that simply is not downloaded.
            return Err(ProviderError::BadRequest {
                provider: PROVIDER_ID,
                detail: format!("{} is not downloaded yet", entry.name),
            });
        }

        let arch = entry.arch.ok_or_else(|| ProviderError::UnknownModel {
            provider: PROVIDER_ID,
            model: model_id.to_string(),
        })?;

        Ok((self.models.directory_for(&entry), arch))
    }
}

#[async_trait]
impl TranscriptionProvider for LocalParakeetProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn name(&self) -> &'static str {
        "Parakeet"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            local: true,
            batch: true,
            // The crate supports streaming; this adapter does not implement it.
            streaming: false,
            timestamps: true,
            word_timestamps: true,
            diarization: false,
            language_detection: false,
            translation: false,
            prompting: false,
        }
    }

    fn models(&self) -> Vec<ModelInfo> {
        self.models
            .installed()
            .into_iter()
            .filter(|status| status.entry.engine == Engine::Parakeet)
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
        "parakeet-tdt-0.6b-v3"
    }

    fn warm_up(&self, model: &str) {
        let Ok((directory, arch)) = self.resolve(model) else {
            return;
        };
        let mut guard = self.loaded.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(error) = ensure_loaded(&mut guard, &directory, arch) {
            tracing::warn!(%error, "could not preload Parakeet");
        }
    }

    fn unload(&self) {
        *self.loaded.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    fn start_capture(
        &self,
        options: crate::providers::capture::CaptureOptions,
    ) -> Option<crate::providers::capture::CaptureTranscription> {
        let (directory, arch) = self.resolve(&options.model).ok()?;
        let loaded = Arc::clone(&self.loaded);
        crate::providers::capture::CaptureTranscription::spawn(move |samples| {
            let audio = samples.iter().map(|&s| s as f32 / 32768.0).collect();
            let started = Instant::now();
            let text = run_parakeet(&loaded, &directory, arch, audio, 16_000)
                .map_err(|e| e.to_string())?;
            let speech = crate::audio::speech::analyze(samples).speech;
            if text.trim().is_empty() || crate::audio::speech::is_phantom_transcript(&text, speech)
            {
                return Err("uncertain phrase; using the full recording".into());
            }
            tracing::info!(
                phrase_ms = started.elapsed().as_millis() as u64,
                audio_ms = samples.len() / 16,
                "Parakeet capture phrase transcribed"
            );
            Ok(text)
        })
    }

    async fn transcribe(
        &self,
        request: TranscriptionRequest,
    ) -> Result<Transcription, ProviderError> {
        let (directory, arch) = self.resolve(&request.model)?;
        let audio = read_wav_as_mono_f32(request.audio.path())?;
        let sample_rate = request.audio.sample_rate;

        let started = Instant::now();
        let model = request.model.clone();

        // ONNX inference is blocking and CPU-heavy; it must not run on an
        // async worker or it would stall every other task in the runtime.
        let loaded = Arc::clone(&self.loaded);
        let text = tauri::async_runtime::spawn_blocking(move || {
            run_parakeet(&loaded, &directory, arch, audio, sample_rate)
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

fn ensure_loaded(
    loaded: &mut Option<Loaded>,
    directory: &std::path::Path,
    arch: ParakeetArch,
) -> Result<(), ProviderError> {
    if loaded
        .as_ref()
        .is_some_and(|l| l.directory == directory && l.arch == arch)
    {
        return Ok(());
    }
    *loaded = None;
    let started = Instant::now();
    let failure = |e: String| ProviderError::BadRequest {
        provider: PROVIDER_ID,
        detail: format!("the model could not be loaded: {e}"),
    };
    // parakeet-rs registers CUDA first and keeps CPU as a required fallback,
    // so an unavailable driver does not make the engine unusable.
    #[cfg(all(target_os = "windows", feature = "cuda"))]
    let config = Some(
        parakeet_rs::ExecutionConfig::new()
            .with_execution_provider(parakeet_rs::ExecutionProvider::Cuda),
    );
    #[cfg(not(all(target_os = "windows", feature = "cuda")))]
    let config = None;

    let model = match arch {
        ParakeetArch::Tdt => ResidentModel::Tdt(
            parakeet_rs::ParakeetTDT::from_pretrained(directory, config)
                .map_err(|e| failure(e.to_string()))?,
        ),
        ParakeetArch::Ctc => ResidentModel::Ctc(
            parakeet_rs::Parakeet::from_pretrained(directory, config)
                .map_err(|e| failure(e.to_string()))?,
        ),
    };
    *loaded = Some(Loaded {
        directory: directory.to_path_buf(),
        arch,
        model,
    });
    tracing::info!(
        load_ms = started.elapsed().as_millis() as u64,
        "Parakeet model loaded"
    );
    Ok(())
}

fn run_parakeet(
    loaded: &Mutex<Option<Loaded>>,
    directory: &std::path::Path,
    arch: ParakeetArch,
    audio: Vec<f32>,
    sample_rate: u32,
) -> Result<String, ProviderError> {
    use parakeet_rs::Transcriber;
    let mut guard = loaded.lock().unwrap_or_else(|e| e.into_inner());
    ensure_loaded(&mut guard, directory, arch)?;
    let result = match &mut guard.as_mut().expect("model loaded").model {
        ResidentModel::Tdt(model) => model.transcribe_samples(audio, sample_rate, 1, None),
        ResidentModel::Ctc(model) => model.transcribe_samples(audio, sample_rate, 1, None),
    };
    match result {
        Ok(result) => Ok(result.text.trim().to_string()),
        Err(error) => {
            // Do not reuse a session that failed partway through inference.
            *guard = None;
            Err(ProviderError::BadRequest {
                provider: PROVIDER_ID,
                detail: format!("transcription failed: {error}"),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "needs CLIDE_BENCH_PARAKEET and CLIDE_BENCH_WAV"]
    async fn bench_parakeet_latency() {
        let directory = PathBuf::from(std::env::var("CLIDE_BENCH_PARAKEET").unwrap());
        let wav = PathBuf::from(std::env::var("CLIDE_BENCH_WAV").unwrap());
        let audio = read_wav_as_mono_f32(&wav).unwrap();
        let arch = if directory.join("encoder-model.onnx").exists() {
            ParakeetArch::Tdt
        } else {
            ParakeetArch::Ctc
        };
        let loaded = Arc::new(Mutex::new(None));
        let mut baseline = String::new();
        for run in 1..=3 {
            let started = Instant::now();
            let text = run_parakeet(&loaded, &directory, arch, audio.clone(), 16_000).unwrap();
            baseline = text.clone();
            println!(
                "PARAKEET run={run} ms={} text={text}",
                started.elapsed().as_millis()
            );
        }
        let engine = Arc::clone(&loaded);
        let run = crate::providers::capture::CaptureTranscription::spawn(move |samples| {
            let pcm = samples.iter().map(|&s| s as f32 / 32768.0).collect();
            run_parakeet(&engine, &directory, arch, pcm, 16_000).map_err(|e| e.to_string())
        })
        .unwrap();
        let tap = run.tap();
        let pcm: Vec<i16> = audio.iter().map(|&s| (s * 32767.0) as i16).collect();
        let playback = Instant::now();
        for (i, frame) in pcm.chunks(320).enumerate() {
            tap(frame);
            let due = std::time::Duration::from_millis((i as u64 + 1) * 20);
            tokio::time::sleep(due.saturating_sub(playback.elapsed())).await;
        }
        drop(tap);
        let released = Instant::now();
        let text = run.finish().await.unwrap();
        let remaining = released.elapsed().as_millis();
        let words = |s: &str| {
            s.to_lowercase()
                .split(|c: char| !c.is_alphanumeric())
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        };
        assert_eq!(words(&text), words(&baseline));
        assert_eq!(
            words(&crate::processing::backtrack::apply_backtracking(&text).text),
            words(&crate::processing::backtrack::apply_backtracking(&baseline).text)
        );
        println!("PARAKEET_CAPTURE release_ms={remaining} text={text}");
        *loaded.lock().unwrap() = None;
    }

    fn provider(name: &str) -> (LocalParakeetProvider, PathBuf) {
        let dir = std::env::temp_dir().join(format!("clide-parakeet-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        (LocalParakeetProvider::new(ModelStore::new(&dir)), dir)
    }

    #[test]
    fn parakeet_declares_itself_local() {
        let (local, _dir) = provider("local");
        assert!(local.capabilities().local);
    }

    #[test]
    fn its_default_model_is_a_parakeet_one() {
        let (local, _dir) = provider("default");
        let entry = catalog::find(local.default_model()).expect("default is in the catalogue");
        assert_eq!(entry.engine, Engine::Parakeet);
    }

    /// Whisper weights must never be offered by the Parakeet engine.
    #[test]
    fn it_only_offers_parakeet_models() {
        let (local, dir) = provider("filter");
        let whisper = catalog::find("whisper-base").unwrap();
        let store = ModelStore::new(&dir);
        store.prepare_directory(&whisper).unwrap();
        for file in &whisper.files {
            std::fs::write(
                store.file_path(&whisper, file),
                vec![0u8; file.bytes as usize],
            )
            .unwrap();
        }

        assert!(
            store.is_installed(&whisper),
            "the whisper model was installed"
        );
        assert!(
            local.models().is_empty(),
            "Parakeet offered a Whisper model"
        );
    }

    #[tokio::test]
    async fn an_uninstalled_model_says_so() {
        let (local, _dir) = provider("uninstalled");
        let request = TranscriptionRequest {
            audio: crate::providers::traits::AudioClip::wav("/nonexistent.wav", 1.0),
            model: "parakeet-tdt-0.6b-v3".into(),
            language: None,
            prompt: None,
        };
        let error = local.transcribe(request).await.unwrap_err();
        assert!(error.to_string().contains("not downloaded"), "got: {error}");
    }
}
