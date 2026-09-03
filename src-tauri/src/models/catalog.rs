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
    /// Bytes on disk once installed. Shown before the download starts.
    pub download_bytes: u64,
    pub speed: SpeedClass,
    pub quality: QualityClass,
    pub multilingual: bool,
    /// Where the weights come from.
    pub url: String,
    /// The file name to store it under, inside the model's own directory.
    pub file_name: String,
    /// SHA-256 of the download, when the source publishes a stable one.
    ///
    /// `None` means Clide can verify that the file arrived complete but not
    /// that it is the exact file expected. Prefer entries that carry one.
    pub sha256: Option<String>,
}

impl CatalogEntry {
    pub fn size_label(&self) -> String {
        let mb = self.download_bytes as f64 / (1024.0 * 1024.0);
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
        CatalogEntry {
            id: "whisper-base".into(),
            name: "Whisper Base".into(),
            engine: Engine::Whisper,
            description: "Small and quick. Good for clear speech and short notes.".into(),
            download_bytes: 147_951_465,
            speed: SpeedClass::Fast,
            quality: QualityClass::Good,
            multilingual: true,
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin".into(),
            file_name: "ggml-base.bin".into(),
            sha256: None,
        },
        CatalogEntry {
            id: "whisper-small".into(),
            name: "Whisper Small".into(),
            engine: Engine::Whisper,
            description: "A noticeable accuracy gain over Base, still fast on Apple Silicon."
                .into(),
            download_bytes: 487_601_967,
            speed: SpeedClass::Fast,
            quality: QualityClass::High,
            multilingual: true,
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin".into(),
            file_name: "ggml-small.bin".into(),
            sha256: None,
        },
        CatalogEntry {
            id: "whisper-large-v3-turbo".into(),
            name: "Whisper Large v3 Turbo".into(),
            engine: Engine::Whisper,
            description: "Cloud-grade accuracy, entirely on this Mac. The best local default."
                .into(),
            download_bytes: 1_624_555_275,
            speed: SpeedClass::Balanced,
            quality: QualityClass::VeryHigh,
            multilingual: true,
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin"
                .into(),
            file_name: "ggml-large-v3-turbo.bin".into(),
            sha256: None,
        },
    ]
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
    fn every_entry_downloads_over_https() {
        for entry in catalog() {
            assert!(
                entry.url.starts_with("https://"),
                "{} would download over plaintext",
                entry.id
            );
        }
    }

    #[test]
    fn every_entry_declares_a_plausible_size() {
        for entry in catalog() {
            assert!(
                entry.download_bytes > 1024 * 1024,
                "{} claims an implausibly small download",
                entry.id
            );
        }
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
