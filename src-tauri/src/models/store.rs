//! Where installed models live on disk, and what is installed right now.
//!
//! The filesystem is the source of truth. A model is installed when its file
//! exists at the expected size — not when a database row says so. That means a
//! model deleted by hand, or a download killed halfway, resolves correctly on
//! the next launch instead of leaving Clide confidently wrong.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::catalog::{self, CatalogEntry, ModelFile};
use super::hardware;
use super::rating::{self, Rating};

/// A catalogue entry plus its state on this machine.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    #[serde(flatten)]
    pub entry: CatalogEntry,
    pub installed: bool,
    /// Bytes present. Below the full size while a download is in flight.
    pub bytes_on_disk: u64,
    pub download_bytes: u64,
    pub size_label: String,
    /// How well this model suits *this* Mac. Derived, never invented.
    pub rating: Rating,
}

#[derive(Clone, Debug)]
pub struct ModelStore {
    root: PathBuf,
}

impl ModelStore {
    /// `data_dir` is the app's data directory; `models/` is created lazily.
    pub fn new(data_dir: &Path) -> Self {
        Self {
            root: data_dir.join("models"),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The directory a model's files live in.
    ///
    /// Engines that take a directory (Parakeet) are handed this; engines that
    /// take a file (whisper.cpp) are handed `path_for`.
    pub fn directory_for(&self, entry: &CatalogEntry) -> PathBuf {
        self.root.join(&entry.id)
    }

    /// Where one of a model's files belongs.
    pub fn file_path(&self, entry: &CatalogEntry, file: &ModelFile) -> PathBuf {
        self.directory_for(entry).join(&file.name)
    }

    /// The primary file, for single-file engines.
    pub fn path_for(&self, entry: &CatalogEntry) -> PathBuf {
        match entry.files.first() {
            Some(file) => self.file_path(entry, file),
            None => self.directory_for(entry),
        }
    }

    /// The path a download writes to before it is complete.
    ///
    /// Downloads land here and are renamed on success, so an interrupted
    /// transfer can never be mistaken for an installed file.
    pub fn partial_path(&self, entry: &CatalogEntry, file: &ModelFile) -> PathBuf {
        self.directory_for(entry)
            .join(format!("{}.partial", file.name))
    }

    /// Total bytes present across every file. Below the full size mid-download.
    pub fn bytes_on_disk(&self, entry: &CatalogEntry) -> u64 {
        entry
            .files
            .iter()
            .map(|file| {
                std::fs::metadata(self.file_path(entry, file))
                    .map(|m| m.len())
                    .unwrap_or(0)
            })
            .sum()
    }

    /// Whether one file is fully present.
    ///
    /// Sizes are compared with a tolerance because published byte counts drift
    /// when a source re-uploads identical weights.
    pub fn has_file(&self, entry: &CatalogEntry, file: &ModelFile) -> bool {
        let actual = std::fs::metadata(self.file_path(entry, file))
            .map(|m| m.len())
            .unwrap_or(0);
        actual > 0 && actual + (file.bytes / 20) >= file.bytes
    }

    /// A model counts as installed only when **every** file is present.
    ///
    /// Parakeet is useless with three of its four artifacts, so a partial set
    /// must never read as installed.
    pub fn is_installed(&self, entry: &CatalogEntry) -> bool {
        !entry.files.is_empty() && entry.files.iter().all(|file| self.has_file(entry, file))
    }

    pub fn status_of(&self, entry: &CatalogEntry) -> ModelStatus {
        ModelStatus {
            installed: self.is_installed(entry),
            bytes_on_disk: self.bytes_on_disk(entry),
            download_bytes: entry.download_bytes(),
            size_label: entry.size_label(),
            rating: rating::rate_local(entry, hardware::hardware()),
            entry: entry.clone(),
        }
    }

    /// The catalogue ordered for the model feed: best fit for this machine
    /// first, then by overall rating. Installed models float to the top,
    /// because what the user already has is what they can use right now.
    pub fn ranked(&self) -> Vec<ModelStatus> {
        let mut all = self.catalog_status();
        all.sort_by(|a, b| {
            b.installed
                .cmp(&a.installed)
                .then(
                    fit_rank(a.rating.fit)
                        .cmp(&fit_rank(b.rating.fit)),
                )
                .then(
                    b.rating
                        .overall
                        .partial_cmp(&a.rating.overall)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
        });
        all
    }

    /// The whole catalogue, annotated with what is on this machine.
    pub fn catalog_status(&self) -> Vec<ModelStatus> {
        catalog::catalog()
            .iter()
            .map(|entry| self.status_of(entry))
            .collect()
    }

    pub fn installed(&self) -> Vec<ModelStatus> {
        self.catalog_status()
            .into_iter()
            .filter(|status| status.installed)
            .collect()
    }

    /// Remove a model's files. Removing something absent is a success.
    pub fn remove(&self, entry: &CatalogEntry) -> std::io::Result<()> {
        let directory = self.root.join(&entry.id);
        match std::fs::remove_dir_all(&directory) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    pub fn prepare_directory(&self, entry: &CatalogEntry) -> std::io::Result<()> {
        std::fs::create_dir_all(self.directory_for(entry))
    }
}

/// Lower is better, so `Great` sorts first.
fn fit_rank(fit: crate::models::Fit) -> u8 {
    use crate::models::Fit;
    match fit {
        Fit::Great => 0,
        Fit::Good => 1,
        Fit::Tight => 2,
        Fit::TooLarge => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(name: &str) -> (ModelStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!("clide-models-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        (ModelStore::new(&dir), dir)
    }

    fn entry() -> CatalogEntry {
        catalog::find("whisper-base").unwrap()
    }

    /// Write every file at full size, as a completed download would.
    fn install(store: &ModelStore, entry: &CatalogEntry) {
        store.prepare_directory(entry).unwrap();
        for file in &entry.files {
            std::fs::write(store.file_path(entry, file), vec![0u8; file.bytes as usize]).unwrap();
        }
    }

    #[test]
    fn nothing_is_installed_in_a_fresh_store() {
        let (store, _dir) = store("fresh");
        assert!(!store.is_installed(&entry()));
        assert!(store.installed().is_empty());
        // The catalogue still lists everything, just uninstalled.
        assert_eq!(store.catalog_status().len(), catalog::catalog().len());
    }

    #[test]
    fn a_complete_file_reads_as_installed() {
        let (store, _dir) = store("installed");
        let entry = entry();
        install(&store, &entry);
        assert!(store.is_installed(&entry));
        assert_eq!(store.installed().len(), 1);
    }

    /// The bug this guards: a half-finished download must never be offered as
    /// a usable model.
    #[test]
    fn a_truncated_download_does_not_count_as_installed() {
        let (store, _dir) = store("truncated");
        let entry = entry();
        let file = &entry.files[0];
        store.prepare_directory(&entry).unwrap();
        std::fs::write(
            store.file_path(&entry, file),
            vec![0u8; (file.bytes / 2) as usize],
        )
        .unwrap();

        assert!(!store.is_installed(&entry));
        assert!(store.bytes_on_disk(&entry) > 0, "the partial bytes are visible");
    }

    /// Parakeet is useless with three of its four artifacts. A model with any
    /// file missing must not read as installed.
    #[test]
    fn a_multi_file_model_needs_every_file() {
        let (store, _dir) = store("multi");
        let parakeet = catalog::find("parakeet-tdt-0.6b-v3").unwrap();
        store.prepare_directory(&parakeet).unwrap();

        // Everything except the last artifact.
        for file in parakeet.files.iter().take(parakeet.files.len() - 1) {
            std::fs::write(
                store.file_path(&parakeet, file),
                vec![0u8; file.bytes as usize],
            )
            .unwrap();
        }
        assert!(!store.is_installed(&parakeet), "a partial set read as installed");

        let last = parakeet.files.last().unwrap();
        std::fs::write(
            store.file_path(&parakeet, last),
            vec![0u8; last.bytes as usize],
        )
        .unwrap();
        assert!(store.is_installed(&parakeet));
    }

    #[test]
    fn a_slightly_smaller_file_is_tolerated() {
        let (store, _dir) = store("tolerance");
        let entry = entry();
        let file = &entry.files[0];
        store.prepare_directory(&entry).unwrap();
        std::fs::write(
            store.file_path(&entry, file),
            vec![0u8; (file.bytes - file.bytes / 40) as usize],
        )
        .unwrap();
        assert!(store.is_installed(&entry));
    }

    #[test]
    fn removing_is_idempotent() {
        let (store, _dir) = store("remove");
        let entry = entry();
        install(&store, &entry);

        store.remove(&entry).unwrap();
        assert!(!store.is_installed(&entry));
        store.remove(&entry).unwrap();
    }

    /// The feed must not lead with something this Mac cannot run.
    #[test]
    fn the_ranked_feed_puts_workable_models_first() {
        let (store, _dir) = store("ranked");
        let ranked = store.ranked();
        assert_eq!(ranked.len(), catalog::catalog().len());

        let ranks: Vec<u8> = ranked.iter().map(|s| fit_rank(s.rating.fit)).collect();
        let mut sorted = ranks.clone();
        sorted.sort();
        assert_eq!(ranks, sorted, "a worse-fitting model was ranked above a better one");
    }

    #[test]
    fn installed_models_lead_the_feed() {
        let (store, _dir) = store("ranked-installed");
        let last = catalog::catalog().pop().unwrap();
        install(&store, &last);

        assert!(store.ranked()[0].installed, "an installed model was not first");
    }

    #[test]
    fn models_do_not_share_a_directory() {
        let (store, _dir) = store("separate");
        let base = catalog::find("whisper-base").unwrap();
        let small = catalog::find("whisper-small").unwrap();
        assert_ne!(store.path_for(&base), store.path_for(&small));
    }

    #[test]
    fn the_partial_path_is_never_the_final_path() {
        let (store, _dir) = store("partial");
        let entry = entry();
        let file = &entry.files[0];
        assert_ne!(store.partial_path(&entry, file), store.file_path(&entry, file));
    }
}
