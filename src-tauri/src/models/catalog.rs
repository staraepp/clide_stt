//! The models Clide knows how to install.
//!
//! Users pick from this list; they never type a file path. `blueprint.md` §13
//! is explicit about that — "Choose model path: /Users/star/Downloads/
//! random-model-v7-final-final.gguf" is the experience this exists to prevent.

use serde::{Deserialize, Serialize};

use crate::providers::traits::{QualityClass, SpeedClass};

/// Which local runtime loads a model. Determines the provider that offers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Engine {
    /// whisper.cpp — GGML weights, Metal accelerated on Apple Silicon.
    Whisper,
    /// NVIDIA Parakeet via ONNX Runtime.
    Parakeet,
}

/// Which Parakeet loader an entry needs.
///
/// The two architectures ship different artifacts and take different code
/// paths in `providers::local::parakeet`, so the catalogue states it rather
/// than the loader guessing from which files happen to be on disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParakeetArch {
    /// Token-and-duration transducer. `encoder-model.onnx`, its `.data`,
    /// `decoder_joint-model.onnx`, `vocab.txt`.
    Tdt,
    /// Connectionist temporal classification. `model.onnx`, its `.onnx_data`,
    /// `tokenizer.json`.
    Ctc,
}

impl Engine {
    pub fn id(self) -> &'static str {
        match self {
            Engine::Whisper => "whisper",
            Engine::Parakeet => "parakeet",
        }
    }
}

/// A model Clide can download, described well enough for someone to choose
/// between two of them without reading a benchmark table.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub engine: Engine,
    pub description: String,
    pub speed: SpeedClass,
    pub quality: QualityClass,
    pub multilingual: bool,
    /// Which Parakeet loader this needs. `None` for every other engine.
    #[serde(default)]
    pub arch: Option<ParakeetArch>,
    /// Everything that must be on disk before the model can load.
    ///
    /// A list rather than a single path because engines disagree: whisper.cpp
    /// takes one `.bin`, Parakeet takes an encoder, its weight blob, a decoder
    /// and a vocabulary, all in one directory.
    pub files: Vec<ModelFile>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelFile {
    /// Name on disk, inside the model's own directory. Engines look for these
    /// exact names, so they are not ours to choose.
    pub name: String,
    pub url: String,
    pub bytes: u64,
    /// SHA-256, when the source publishes a stable one.
    ///
    /// `None` means Clide can verify the file arrived complete but not that it
    /// is the exact file expected. Prefer entries that carry one.
    pub sha256: Option<String>,
}

impl CatalogEntry {
    /// Total bytes across every file. Shown before a download starts.
    pub fn download_bytes(&self) -> u64 {
        self.files.iter().map(|file| file.bytes).sum()
    }

    pub fn size_label(&self) -> String {
        let mb = self.download_bytes() as f64 / (1024.0 * 1024.0);
        if mb >= 1024.0 {
            format!("{:.1} GB", mb / 1024.0)
        } else {
            format!("{mb:.0} MB")
        }
    }
}

/// Everything installable in this build.
///
/// Whisper weights come from ggerganov's `whisper.cpp` repository on Hugging
/// Face, which is the canonical source for the GGML conversions.
pub fn catalog() -> Vec<CatalogEntry> {
    vec![
        // --- Whisper, smallest first -------------------------------------
        whisper(
            "whisper-tiny",
            "Whisper Tiny",
            "Tiny and instant. Fine for short, clear notes.",
            "ggml-tiny.bin",
            77_691_713,
            SpeedClass::Fast,
            QualityClass::Good,
        ),
        whisper(
            "whisper-base",
            "Whisper Base",
            "Small and quick. Good for clear speech and short notes.",
            "ggml-base.bin",
            147_951_465,
            SpeedClass::Fast,
            QualityClass::Good,
        ),
        whisper(
            "whisper-small-q5",
            "Whisper Small (compressed)",
            "Small's accuracy at a third of the size. A good first download.",
            "ggml-small-q5_1.bin",
            190_085_487,
            SpeedClass::Fast,
            QualityClass::High,
        ),
        whisper(
            "whisper-small",
            "Whisper Small",
            "A clear accuracy gain over Base, still fast on Apple Silicon.",
            "ggml-small.bin",
            487_601_967,
            SpeedClass::Fast,
            QualityClass::High,
        ),
        whisper(
            "whisper-medium-q5",
            "Whisper Medium (compressed)",
            "Medium's accuracy in a third of the space.",
            "ggml-medium-q5_0.bin",
            539_212_467,
            SpeedClass::Balanced,
            QualityClass::High,
        ),
        whisper(
            "whisper-large-v3-turbo-q5",
            "Whisper Large v3 Turbo (compressed)",
            "The best trade in the catalogue: Turbo's accuracy at a third the size.",
            "ggml-large-v3-turbo-q5_0.bin",
            574_041_195,
            SpeedClass::Fast,
            QualityClass::VeryHigh,
        ),
        whisper(
            "whisper-large-v3-q5",
            "Whisper Large v3 (compressed)",
            "Full Large v3 accuracy, compressed. Slower than Turbo.",
            "ggml-large-v3-q5_0.bin",
            1_081_140_203,
            SpeedClass::Thorough,
            QualityClass::VeryHigh,
        ),
        whisper(
            "whisper-medium",
            "Whisper Medium",
            "Uncompressed Medium. Prefer the compressed build unless comparing.",
            "ggml-medium.bin",
            1_533_763_059,
            SpeedClass::Balanced,
            QualityClass::High,
        ),
        whisper(
            "whisper-large-v3-turbo",
            "Whisper Large v3 Turbo",
            "Cloud-grade accuracy, entirely on this Mac.",
            "ggml-large-v3-turbo.bin",
            1_624_555_275,
            SpeedClass::Balanced,
            QualityClass::VeryHigh,
        ),
        // --- Parakeet -----------------------------------------------------
        CatalogEntry {
            id: "parakeet-ctc-0.6b-int8".into(),
            name: "Parakeet CTC 0.6B (compressed)".into(),
            engine: Engine::Parakeet,
            arch: Some(ParakeetArch::Ctc),
            description: "NVIDIA's CTC model, quantised. Very fast, English-first.".into(),
            speed: SpeedClass::Fast,
            quality: QualityClass::High,
            multilingual: false,
            files: vec![
                ctc_file("model.onnx", "onnx/model_int8.onnx", 1_303_007),
                ctc_file("model.onnx_data", "onnx/model_int8.onnx_data", 610_974_468),
                ctc_file("tokenizer.json", "tokenizer.json", 412_363),
            ],
        },
        CatalogEntry {
            id: "parakeet-ctc-0.6b".into(),
            name: "Parakeet CTC 0.6B".into(),
            engine: Engine::Parakeet,
            arch: Some(ParakeetArch::Ctc),
            description: "Full-precision CTC. Accurate and quick on Apple Silicon.".into(),
            speed: SpeedClass::Fast,
            quality: QualityClass::VeryHigh,
            multilingual: false,
            files: vec![
                ctc_file("model.onnx", "onnx/model.onnx", 887_486),
                ctc_file("model.onnx_data", "onnx/model.onnx_data", 2_435_004_420),
                ctc_file("tokenizer.json", "tokenizer.json", 412_363),
            ],
        },
        CatalogEntry {
            id: "parakeet-tdt-0.6b-v3".into(),
            name: "Parakeet TDT 0.6B".into(),
            engine: Engine::Parakeet,
            arch: Some(ParakeetArch::Tdt),
            description: "NVIDIA's transducer. Strong on conversational speech, 25 languages."
                .into(),
            speed: SpeedClass::Fast,
            quality: QualityClass::VeryHigh,
            multilingual: true,
            files: vec![
                tdt_file("encoder-model.onnx", 41_770_866),
                tdt_file("encoder-model.onnx.data", 2_435_420_160),
                tdt_file("decoder_joint-model.onnx", 72_520_893),
                tdt_file("vocab.txt", 93_939),
            ],
        },
    ]
}

/// GGML weights, from the canonical whisper.cpp conversions.
fn whisper(
    id: &str,
    name: &str,
    description: &str,
    file_name: &str,
    bytes: u64,
    speed: SpeedClass,
    quality: QualityClass,
) -> CatalogEntry {
    CatalogEntry {
        id: id.into(),
        name: name.into(),
        engine: Engine::Whisper,
        arch: None,
        description: description.into(),
        speed,
        quality,
        multilingual: true,
        files: vec![ModelFile {
            name: file_name.into(),
            url: format!(
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{file_name}"
            ),
            bytes,
            sha256: None,
        }],
    }
}

fn tdt_file(name: &str, bytes: u64) -> ModelFile {
    ModelFile {
        name: name.into(),
        url: format!(
            "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/{name}"
        ),
        bytes,
        sha256: None,
    }
}

/// The CTC repository nests its weights under `onnx/`, and the quantised
/// builds carry a suffix — but the loader expects fixed names, so `name` and
/// the remote path deliberately differ.
fn ctc_file(name: &str, remote: &str, bytes: u64) -> ModelFile {
    ModelFile {
        name: name.into(),
        url: format!(
            "https://huggingface.co/onnx-community/parakeet-ctc-0.6b-ONNX/resolve/main/{remote}"
        ),
        bytes,
        sha256: None,
    }
}

pub fn find(id: &str) -> Option<CatalogEntry> {
    catalog().into_iter().find(|entry| entry.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_entry_has_a_unique_id() {
        let ids: HashSet<_> = catalog().into_iter().map(|e| e.id).collect();
        assert_eq!(ids.len(), catalog().len());
    }

    #[test]
    fn every_file_downloads_over_https() {
        for entry in catalog() {
            assert!(!entry.files.is_empty(), "{} has no files", entry.id);
            for file in &entry.files {
                assert!(
                    file.url.starts_with("https://"),
                    "{}/{} would download over plaintext",
                    entry.id,
                    file.name
                );
            }
        }
    }

    #[test]
    fn every_entry_declares_a_plausible_size() {
        for entry in catalog() {
            assert!(
                entry.download_bytes() > 1024 * 1024,
                "{} claims an implausibly small download",
                entry.id
            );
        }
    }

    /// Engines look for exact file names, so a duplicate would silently
    /// overwrite one of them during download.
    #[test]
    fn file_names_are_unique_within_an_entry() {
        for entry in catalog() {
            let mut names: Vec<_> = entry.files.iter().map(|f| &f.name).collect();
            names.sort();
            let count = names.len();
            names.dedup();
            assert_eq!(count, names.len(), "{} repeats a file name", entry.id);
        }
    }

    /// Every Parakeet entry must name its loader; nothing else may.
    #[test]
    fn only_parakeet_entries_declare_an_architecture() {
        for entry in catalog() {
            match entry.engine {
                Engine::Parakeet => assert!(
                    entry.arch.is_some(),
                    "{} is a Parakeet model with no architecture",
                    entry.id
                ),
                Engine::Whisper => assert!(
                    entry.arch.is_none(),
                    "{} is not Parakeet but declares an architecture",
                    entry.id
                ),
            }
        }
    }

    /// The CTC repo nests weights under `onnx/` and suffixes quantised builds,
    /// but the loader wants fixed names — so these must differ.
    #[test]
    fn ctc_files_are_stored_under_the_names_the_loader_expects() {
        let ctc = find("parakeet-ctc-0.6b-int8").unwrap();
        let names: Vec<&str> = ctc.files.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"model.onnx"));
        assert!(names.contains(&"model.onnx_data"));
        assert!(names.contains(&"tokenizer.json"));

        let weights = ctc.files.iter().find(|f| f.name == "model.onnx_data").unwrap();
        assert!(weights.url.contains("model_int8.onnx_data"));
    }

    #[test]
    fn the_catalogue_spans_a_useful_range_of_sizes() {
        let sizes: Vec<u64> = catalog().iter().map(|e| e.download_bytes()).collect();
        let smallest = *sizes.iter().min().unwrap();
        let largest = *sizes.iter().max().unwrap();
        assert!(smallest < 100 * 1024 * 1024, "nothing small enough to try");
        assert!(largest > 1024 * 1024 * 1024, "nothing accurate enough to keep");
    }

    #[test]
    fn the_multi_file_entry_sums_its_parts() {
        let parakeet = find("parakeet-tdt-0.6b-v3").unwrap();
        assert_eq!(parakeet.files.len(), 4);
        assert_eq!(
            parakeet.download_bytes(),
            parakeet.files.iter().map(|f| f.bytes).sum::<u64>()
        );
    }

    #[test]
    fn sizes_read_as_something_a_person_can_judge() {
        let entry = find("whisper-base").unwrap();
        assert_eq!(entry.size_label(), "141 MB");
        assert_eq!(find("whisper-large-v3-turbo").unwrap().size_label(), "1.5 GB");
    }

    #[test]
    fn an_unknown_id_is_none_rather_than_a_panic() {
        assert!(find("whisper-imaginary").is_none());
    }
}
