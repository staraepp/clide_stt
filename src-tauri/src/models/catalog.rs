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
            "whisper-tiny-q5",
            "Whisper Tiny Q5",
            "The smallest multilingual download. Best for quick, clear notes.",
            "ggml-tiny-q5_1.bin",
            32_152_673,
            SpeedClass::Fast,
            QualityClass::Good,
        ),
        whisper_en(
            "whisper-tiny-en-q5",
            "Whisper Tiny English Q5",
            "The smallest English-only option for near-instant dictation.",
            "ggml-tiny.en-q5_1.bin",
            32_166_155,
            SpeedClass::Fast,
            QualityClass::Good,
        ),
        whisper(
            "whisper-tiny-q8",
            "Whisper Tiny Q8",
            "A compact multilingual Tiny build with lighter quantisation.",
            "ggml-tiny-q8_0.bin",
            43_537_433,
            SpeedClass::Fast,
            QualityClass::Good,
        ),
        whisper_en(
            "whisper-tiny-en-q8",
            "Whisper Tiny English Q8",
            "A compact English-only Tiny build with lighter quantisation.",
            "ggml-tiny.en-q8_0.bin",
            43_550_795,
            SpeedClass::Fast,
            QualityClass::Good,
        ),
        whisper(
            "whisper-base-q5",
            "Whisper Base Q5",
            "A small multilingual Base build with a low storage footprint.",
            "ggml-base-q5_1.bin",
            59_707_625,
            SpeedClass::Fast,
            QualityClass::Good,
        ),
        whisper_en(
            "whisper-base-en-q5",
            "Whisper Base English Q5",
            "A small English-only Base build for fast everyday dictation.",
            "ggml-base.en-q5_1.bin",
            59_721_011,
            SpeedClass::Fast,
            QualityClass::Good,
        ),
        whisper(
            "whisper-tiny",
            "Whisper Tiny",
            "Tiny and instant. Fine for short, clear notes.",
            "ggml-tiny.bin",
            77_691_713,
            SpeedClass::Fast,
            QualityClass::Good,
        ),
        whisper_en(
            "whisper-tiny-en",
            "Whisper Tiny English",
            "The full Tiny model tuned for English-only speech.",
            "ggml-tiny.en.bin",
            77_704_715,
            SpeedClass::Fast,
            QualityClass::Good,
        ),
        whisper(
            "whisper-base-q8",
            "Whisper Base Q8",
            "A compact multilingual Base build with lighter quantisation.",
            "ggml-base-q8_0.bin",
            81_768_585,
            SpeedClass::Fast,
            QualityClass::Good,
        ),
        whisper_en(
            "whisper-base-en-q8",
            "Whisper Base English Q8",
            "A compact English-only Base build with lighter quantisation.",
            "ggml-base.en-q8_0.bin",
            81_781_811,
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
        whisper_en(
            "whisper-base-en",
            "Whisper Base English",
            "The full Base model tuned for English-only speech.",
            "ggml-base.en.bin",
            147_964_211,
            SpeedClass::Fast,
            QualityClass::Good,
        ),
        whisper(
            "whisper-small-q5",
            "Whisper Small Q5",
            "A compressed multilingual Small build. A good first download.",
            "ggml-small-q5_1.bin",
            190_085_487,
            SpeedClass::Fast,
            QualityClass::High,
        ),
        whisper_en(
            "whisper-small-en-q5",
            "Whisper Small English Q5",
            "A compressed English-only Small build with strong accuracy.",
            "ggml-small.en-q5_1.bin",
            190_098_681,
            SpeedClass::Fast,
            QualityClass::High,
        ),
        whisper(
            "whisper-small-q8",
            "Whisper Small Q8",
            "Small with lighter quantisation for more detail at a modest size.",
            "ggml-small-q8_0.bin",
            264_464_607,
            SpeedClass::Fast,
            QualityClass::High,
        ),
        whisper_en(
            "whisper-small-en-q8",
            "Whisper Small English Q8",
            "English-only Small with lighter quantisation and strong accuracy.",
            "ggml-small.en-q8_0.bin",
            264_477_561,
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
        whisper_en(
            "whisper-small-en",
            "Whisper Small English",
            "The full Small model tuned for accurate English dictation.",
            "ggml-small.en.bin",
            487_614_201,
            SpeedClass::Fast,
            QualityClass::High,
        ),
        whisper(
            "whisper-medium-q5",
            "Whisper Medium Q5",
            "A compressed multilingual Medium build for accurate dictation.",
            "ggml-medium-q5_0.bin",
            539_212_467,
            SpeedClass::Balanced,
            QualityClass::High,
        ),
        whisper_en(
            "whisper-medium-en-q5",
            "Whisper Medium English Q5",
            "A compressed English-only Medium build for accurate dictation.",
            "ggml-medium.en-q5_0.bin",
            539_225_533,
            SpeedClass::Balanced,
            QualityClass::High,
        ),
        whisper(
            "whisper-large-v3-turbo-q5",
            "Whisper Large v3 Turbo Q5",
            "The smallest Turbo build and a strong speed-to-quality tradeoff.",
            "ggml-large-v3-turbo-q5_0.bin",
            574_041_195,
            SpeedClass::Fast,
            QualityClass::VeryHigh,
        ),
        whisper(
            "whisper-medium-q8",
            "Whisper Medium Q8",
            "Medium with lighter quantisation for more detail than Q5.",
            "ggml-medium-q8_0.bin",
            823_369_779,
            SpeedClass::Balanced,
            QualityClass::High,
        ),
        whisper_en(
            "whisper-medium-en-q8",
            "Whisper Medium English Q8",
            "English-only Medium with lighter quantisation than Q5.",
            "ggml-medium.en-q8_0.bin",
            823_382_461,
            SpeedClass::Balanced,
            QualityClass::High,
        ),
        whisper(
            "whisper-large-v3-turbo-q8",
            "Whisper Large v3 Turbo Q8",
            "Turbo with lighter quantisation for a quality-focused compact build.",
            "ggml-large-v3-turbo-q8_0.bin",
            874_188_075,
            SpeedClass::Balanced,
            QualityClass::VeryHigh,
        ),
        whisper(
            "whisper-large-v3-q5",
            "Whisper Large v3 Q5",
            "A compressed Large v3 build. Slower and more thorough than Turbo.",
            "ggml-large-v3-q5_0.bin",
            1_081_140_203,
            SpeedClass::Thorough,
            QualityClass::VeryHigh,
        ),
        whisper(
            "whisper-large-v2-q5",
            "Whisper Large v2 Q5",
            "A compressed legacy Large v2 build for comparison and compatibility.",
            "ggml-large-v2-q5_0.bin",
            1_080_732_091,
            SpeedClass::Thorough,
            QualityClass::VeryHigh,
        ),
        whisper(
            "whisper-medium",
            "Whisper Medium",
            "The full multilingual Medium model.",
            "ggml-medium.bin",
            1_533_763_059,
            SpeedClass::Balanced,
            QualityClass::High,
        ),
        whisper_en(
            "whisper-medium-en",
            "Whisper Medium English",
            "The full Medium model tuned for English-only speech.",
            "ggml-medium.en.bin",
            1_533_774_781,
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
        whisper(
            "whisper-large-v2-q8",
            "Whisper Large v2 Q8",
            "A lightly quantised legacy Large v2 build for compatibility testing.",
            "ggml-large-v2-q8_0.bin",
            1_656_129_691,
            SpeedClass::Thorough,
            QualityClass::VeryHigh,
        ),
        whisper(
            "whisper-large-v1",
            "Whisper Large v1",
            "The original full Large model, retained for legacy comparisons.",
            "ggml-large-v1.bin",
            3_094_623_691,
            SpeedClass::Thorough,
            QualityClass::High,
        ),
        whisper(
            "whisper-large-v2",
            "Whisper Large v2",
            "The full legacy Large v2 model for compatibility and comparison.",
            "ggml-large-v2.bin",
            3_094_623_691,
            SpeedClass::Thorough,
            QualityClass::VeryHigh,
        ),
        whisper(
            "whisper-large-v3",
            "Whisper Large v3",
            "The full multilingual Large v3 model. Highest fidelity, largest download.",
            "ggml-large-v3.bin",
            3_095_033_483,
            SpeedClass::Thorough,
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
            id: "parakeet-tdt-0.6b-v3-int8".into(),
            name: "Parakeet TDT 0.6B (compressed)".into(),
            engine: Engine::Parakeet,
            arch: Some(ParakeetArch::Tdt),
            description: "The same transducer at a quarter of the size. The best \
                          Parakeet to start with."
                .into(),
            speed: SpeedClass::Fast,
            quality: QualityClass::High,
            multilingual: true,
            // The quantised encoder is a single file — there is no companion
            // `.data` blob, which is where most of the saving comes from.
            files: vec![
                tdt_file_as("encoder-model.onnx", "v3", "encoder-model.int8.onnx", 652_183_999),
                tdt_file_as(
                    "decoder_joint-model.onnx",
                    "v3",
                    "decoder_joint-model.int8.onnx",
                    18_202_004,
                ),
                tdt_file_as("vocab.txt", "v3", "vocab.txt", 93_939),
            ],
        },
        CatalogEntry {
            id: "parakeet-tdt-0.6b-v2-int8".into(),
            name: "Parakeet TDT 0.6B v2 (compressed)".into(),
            engine: Engine::Parakeet,
            arch: Some(ParakeetArch::Tdt),
            description: "The English-focused generation, quantised. Very fast, and \
                          strong on dictation."
                .into(),
            speed: SpeedClass::Fast,
            quality: QualityClass::High,
            multilingual: false,
            files: vec![
                tdt_file_as("encoder-model.onnx", "v2", "encoder-model.int8.onnx", 652_184_014),
                tdt_file_as(
                    "decoder_joint-model.onnx",
                    "v2",
                    "decoder_joint-model.int8.onnx",
                    8_998_286,
                ),
                tdt_file_as("vocab.txt", "v2", "vocab.txt", 9_384),
            ],
        },
        CatalogEntry {
            id: "parakeet-tdt-0.6b-v2".into(),
            name: "Parakeet TDT 0.6B v2".into(),
            engine: Engine::Parakeet,
            arch: Some(ParakeetArch::Tdt),
            description: "Full-precision English transducer. Accurate on \
                          conversational speech."
                .into(),
            speed: SpeedClass::Fast,
            quality: QualityClass::VeryHigh,
            multilingual: false,
            files: vec![
                tdt_file_as("encoder-model.onnx", "v2", "encoder-model.onnx", 41_770_866),
                tdt_file_as(
                    "encoder-model.onnx.data",
                    "v2",
                    "encoder-model.onnx.data",
                    2_435_420_160,
                ),
                tdt_file_as(
                    "decoder_joint-model.onnx",
                    "v2",
                    "decoder_joint-model.onnx",
                    35_792_059,
                ),
                tdt_file_as("vocab.txt", "v2", "vocab.txt", 9_384),
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
    whisper_entry(
        id,
        name,
        description,
        file_name,
        bytes,
        speed,
        quality,
        true,
    )
}

/// English-only GGML weights. These trade automatic language support for a
/// model tuned specifically for English speech.
fn whisper_en(
    id: &str,
    name: &str,
    description: &str,
    file_name: &str,
    bytes: u64,
    speed: SpeedClass,
    quality: QualityClass,
) -> CatalogEntry {
    whisper_entry(
        id,
        name,
        description,
        file_name,
        bytes,
        speed,
        quality,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn whisper_entry(
    id: &str,
    name: &str,
    description: &str,
    file_name: &str,
    bytes: u64,
    speed: SpeedClass,
    quality: QualityClass,
    multilingual: bool,
) -> CatalogEntry {
    CatalogEntry {
        id: id.into(),
        name: name.into(),
        engine: Engine::Whisper,
        arch: None,
        description: description.into(),
        speed,
        quality,
        multilingual,
        files: vec![ModelFile {
            name: file_name.into(),
            url: format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{file_name}"),
            bytes,
            sha256: None,
        }],
    }
}

fn tdt_file(name: &str, bytes: u64) -> ModelFile {
    tdt_file_as(name, "v3", name, bytes)
}

/// A TDT artifact, stored under the name `ParakeetTDT::from_pretrained`
/// expects.
///
/// `remote` differs from `name` for the quantised builds — they are published
/// as `encoder-model.int8.onnx`, but the loader only looks for
/// `encoder-model.onnx`.
fn tdt_file_as(name: &str, generation: &str, remote: &str, bytes: u64) -> ModelFile {
    ModelFile {
        name: name.into(),
        url: format!(
            "https://huggingface.co/istupakov/parakeet-tdt-0.6b-{generation}-onnx/resolve/main/{remote}"
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
    fn the_catalogue_includes_the_complete_canonical_ggml_family() {
        let whisper_entries: Vec<_> = catalog()
            .into_iter()
            .filter(|entry| entry.engine == Engine::Whisper)
            .collect();
        assert_eq!(whisper_entries.len(), 33);

        let files: HashSet<_> = whisper_entries
            .iter()
            .flat_map(|entry| entry.files.iter().map(|file| file.name.as_str()))
            .collect();
        for expected in [
            "ggml-tiny.en-q5_1.bin",
            "ggml-base-q8_0.bin",
            "ggml-small.en.bin",
            "ggml-medium.en-q8_0.bin",
            "ggml-large-v1.bin",
            "ggml-large-v2-q5_0.bin",
            "ggml-large-v3.bin",
            "ggml-large-v3-turbo-q8_0.bin",
        ] {
            assert!(
                files.contains(expected),
                "missing canonical model {expected}"
            );
        }
    }

    #[test]
    fn english_only_weights_do_not_claim_multilingual_support() {
        for entry in catalog()
            .into_iter()
            .filter(|entry| entry.engine == Engine::Whisper)
        {
            let english_only = entry.files[0].name.contains(".en");
            assert_eq!(
                entry.multilingual, !english_only,
                "{} has the wrong language capability",
                entry.id
            );
        }
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
    /// The quantised TDT builds are published with an `.int8.` infix, but
    /// `ParakeetTDT::from_pretrained` only looks for the plain names. If these
    /// ever match, the loader will not find its weights.
    #[test]
    fn quantised_tdt_files_are_stored_under_the_plain_loader_names() {
        let compressed = find("parakeet-tdt-0.6b-v3-int8").unwrap();

        let names: Vec<&str> = compressed.files.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"encoder-model.onnx"));
        assert!(names.contains(&"decoder_joint-model.onnx"));
        assert!(names.iter().all(|n| !n.contains("int8")));

        let encoder = compressed
            .files
            .iter()
            .find(|f| f.name == "encoder-model.onnx")
            .unwrap();
        assert!(encoder.url.contains("encoder-model.int8.onnx"));

        // The saving comes from the quantised encoder having no companion
        // blob; if a `.data` file appears here the size claim is wrong.
        assert!(
            !names.contains(&"encoder-model.onnx.data"),
            "the quantised build should not carry a weights blob"
        );
    }

    /// v2 and v3 are different repositories. Mixing their artifacts would load
    /// an encoder against the wrong vocabulary.
    #[test]
    fn each_parakeet_entry_draws_from_a_single_repository() {
        for entry in catalog() {
            if entry.engine != Engine::Parakeet {
                continue;
            }
            let repos: std::collections::HashSet<&str> = entry
                .files
                .iter()
                .map(|f| {
                    f.url
                        .trim_start_matches("https://huggingface.co/")
                        .split("/resolve/")
                        .next()
                        .unwrap_or_default()
                })
                .collect();
            assert_eq!(repos.len(), 1, "{} mixes repositories: {repos:?}", entry.id);
        }
    }

    #[test]
    fn ctc_files_are_stored_under_the_names_the_loader_expects() {
        let ctc = find("parakeet-ctc-0.6b-int8").unwrap();
        let names: Vec<&str> = ctc.files.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"model.onnx"));
        assert!(names.contains(&"model.onnx_data"));
        assert!(names.contains(&"tokenizer.json"));

        let weights = ctc
            .files
            .iter()
            .find(|f| f.name == "model.onnx_data")
            .unwrap();
        assert!(weights.url.contains("model_int8.onnx_data"));
    }

    #[test]
    fn the_catalogue_spans_a_useful_range_of_sizes() {
        let sizes: Vec<u64> = catalog().iter().map(|e| e.download_bytes()).collect();
        let smallest = *sizes.iter().min().unwrap();
        let largest = *sizes.iter().max().unwrap();
        assert!(smallest < 100 * 1024 * 1024, "nothing small enough to try");
        assert!(
            largest > 1024 * 1024 * 1024,
            "nothing accurate enough to keep"
        );
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
        assert_eq!(
            find("whisper-large-v3-turbo").unwrap().size_label(),
            "1.5 GB"
        );
    }

    #[test]
    fn an_unknown_id_is_none_rather_than_a_panic() {
        assert!(find("whisper-imaginary").is_none());
    }
}
