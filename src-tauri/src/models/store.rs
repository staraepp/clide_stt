//! Where installed models live on disk, and what is installed right now.
//!
//! The filesystem is the source of truth. A model is installed when its file
//! exists at the expected size — not when a database row says so. That means a
//! model deleted by hand, or a download killed halfway, resolves correctly on
//! the next launch instead of leaving Clide confidently wrong.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::catalog::{self, CatalogEntry};

/// A catalogue entry plus its state on this machine.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    #[serde(flatten)]
    pub entry: CatalogEntry,
    pub installed: bool,
    /// Bytes present. Below `download_bytes` while a download is in flight.
    pub bytes_on_disk: u64,
    pub size_label: String,
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

    /// Where a model's weights belong. One directory per model so a future
    /// multi-file engine does not need this layout to change.
    pub fn path_for(&self, entry: &CatalogEntry) -> PathBuf {
        self.root.join(&entry.id).join(&entry.file_name)
    }

    /// The path a download writes to before it is complete.
    ///
    /// Downloads land here and are renamed on success, so an interrupted
    /// transfer can never be mistaken for an installed model.
    pub fn partial_path_for(&self, entry: &CatalogEntry) -> PathBuf {
        self.root
            .join(&entry.id)
            .join(format!("{}.partial", entry.file_name))
    }

    pub fn bytes_on_disk(&self, entry: &CatalogEntry) -> u64 {
        std::fs::metadata(self.path_for(entry))
            .map(|m| m.len())
            .unwrap_or(0)
    }

    /// A model counts as installed when the full file is present.
    ///
    /// The size is compared with a tolerance because published byte counts
    /// drift when a source re-uploads identical weights.
    pub fn is_installed(&self, entry: &CatalogEntry) -> bool {
        let actual = self.bytes_on_disk(entry);
        if actual == 0 {
            return false;
        }
        let expected = entry.download_bytes;
        let tolerance = expected / 20; // 5%
        actual + tolerance >= expected
    }

    pub fn status_of(&self, entry: &CatalogEntry) -> ModelStatus {
        ModelStatus {
            installed: self.is_installed(entry),
            bytes_on_disk: self.bytes_on_disk(entry),
            size_label: entry.size_label(),
            entry: entry.clone(),
        }
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
        std::fs::create_dir_all(self.root.join(&entry.id))
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

    fn write(store: &ModelStore, entry: &CatalogEntry, bytes: u64) {
        store.prepare_directory(entry).unwrap();
        std::fs::write(store.path_for(entry), vec![0u8; bytes as usize]).unwrap();
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
        write(&store, &entry, entry.download_bytes);
        assert!(store.is_installed(&entry));
        assert_eq!(store.installed().len(), 1);
    }

    /// The bug this guards: a half-finished download must never be offered as
    /// a usable model.
    #[test]
    fn a_truncated_download_does_not_count_as_installed() {
        let (store, _dir) = store("truncated");
        let entry = entry();
        write(&store, &entry, entry.download_bytes / 2);
        assert!(!store.is_installed(&entry));
        assert!(store.bytes_on_disk(&entry) > 0, "the partial file is still visible");
    }

    #[test]
    fn a_slightly_smaller_file_is_tolerated() {
        let (store, _dir) = store("tolerance");
        let entry = entry();
        write(&store, &entry, entry.download_bytes - (entry.download_bytes / 40));
        assert!(store.is_installed(&entry));
    }

    #[test]
    fn removing_is_idempotent() {
        let (store, _dir) = store("remove");
        let entry = entry();
        write(&store, &entry, entry.download_bytes);

        store.remove(&entry).unwrap();
        assert!(!store.is_installed(&entry));
        store.remove(&entry).unwrap();
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
        assert_ne!(store.partial_path_for(&entry), store.path_for(&entry));
    }
}
