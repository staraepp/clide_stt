//! Downloading model weights, with progress the UI can render.
//!
//! Downloads stream to a `.partial` file and are renamed only once the whole
//! body has arrived, so an interrupted transfer can never be mistaken for an
//! installed model. Progress is emitted as Tauri events rather than polled.

use std::io::Write;

use futures_util::StreamExt;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::catalog::CatalogEntry;
use super::store::ModelStore;

pub const EVENT_PROGRESS: &str = "model:progress";
pub const EVENT_COMPLETE: &str = "model:complete";
pub const EVENT_FAILED: &str = "model:failed";

/// Emitted often enough to animate, rarely enough not to flood the webview.
const PROGRESS_INTERVAL_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub model_id: String,
    pub received_bytes: u64,
    pub total_bytes: u64,
    /// 0..1. Falls back to the catalogue size when the server sends no length.
    pub fraction: f32,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failed {
    pub model_id: String,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("could not reach the model host: {0}")]
    Network(String),

    #[error("the model host returned {0}")]
    Status(u16),

    #[error("the download could not be written to disk: {0}")]
    Disk(String),

    #[error("the downloaded file did not match its checksum")]
    ChecksumMismatch,
}

/// Fetch a model's weights, emitting progress as they arrive.
///
/// Cancellation is not modelled: a dropped future stops the transfer and leaves
/// the `.partial` file, which the next attempt overwrites.
pub async fn download(
    app: &AppHandle,
    store: &ModelStore,
    entry: &CatalogEntry,
    http: &reqwest::Client,
) -> Result<(), DownloadError> {
    store
        .prepare_directory(entry)
        .map_err(|e| DownloadError::Disk(e.to_string()))?;

    let response = http
        .get(&entry.url)
        .send()
        .await
        .map_err(|e| DownloadError::Network(e.to_string()))?;

    if !response.status().is_success() {
        return Err(DownloadError::Status(response.status().as_u16()));
    }

    let total = response.content_length().unwrap_or(entry.download_bytes);
    let partial = store.partial_path_for(entry);

    let mut file =
        std::fs::File::create(&partial).map_err(|e| DownloadError::Disk(e.to_string()))?;

    let mut hasher = entry.sha256.as_ref().map(|_| Sha256Writer::new());
    let mut received: u64 = 0;
    let mut last_emit: u64 = 0;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| DownloadError::Network(e.to_string()))?;

        file.write_all(&chunk)
            .map_err(|e| DownloadError::Disk(e.to_string()))?;
        if let Some(hasher) = hasher.as_mut() {
            hasher.update(&chunk);
        }

        received += chunk.len() as u64;

        if received - last_emit >= PROGRESS_INTERVAL_BYTES {
            last_emit = received;
            emit_progress(app, entry, received, total);
        }
    }

    file.flush().map_err(|e| DownloadError::Disk(e.to_string()))?;
    drop(file);

    if let (Some(hasher), Some(expected)) = (hasher, entry.sha256.as_ref()) {
        if !hasher.finish().eq_ignore_ascii_case(expected) {
            let _ = std::fs::remove_file(&partial);
            return Err(DownloadError::ChecksumMismatch);
        }
    }

    // The rename is what marks the model installed, and it is atomic.
    std::fs::rename(&partial, store.path_for(entry))
        .map_err(|e| DownloadError::Disk(e.to_string()))?;

    emit_progress(app, entry, total, total);
    let _ = app.emit(EVENT_COMPLETE, entry.id.clone());
    Ok(())
}

fn emit_progress(app: &AppHandle, entry: &CatalogEntry, received: u64, total: u64) {
    let fraction = if total > 0 {
        (received as f32 / total as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let _ = app.emit(
        EVENT_PROGRESS,
        Progress {
            model_id: entry.id.clone(),
            received_bytes: received,
            total_bytes: total,
            fraction,
        },
    );
}

/// Minimal SHA-256, so verifying a download does not add a dependency.
struct Sha256Writer {
    state: [u32; 8],
    buffer: Vec<u8>,
    length: u64,
}

impl Sha256Writer {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c,
                0x1f83d9ab, 0x5be0cd19,
            ],
            buffer: Vec::with_capacity(64),
            length: 0,
        }
    }

    fn update(&mut self, data: &[u8]) {
        self.length += data.len() as u64;
        self.buffer.extend_from_slice(data);
        while self.buffer.len() >= 64 {
            let block: [u8; 64] = self.buffer[..64].try_into().unwrap();
            self.compress(&block);
            self.buffer.drain(..64);
        }
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for (index, word) in w.iter_mut().take(16).enumerate() {
            let start = index * 4;
            *word = u32::from_be_bytes(block[start..start + 4].try_into().unwrap());
        }
        // The message schedule is defined by back-references, so this one
        // genuinely needs indices rather than an iterator.
        #[allow(clippy::needless_range_loop)]
        for index in 16..64 {
            let s0 = w[index - 15].rotate_right(7)
                ^ w[index - 15].rotate_right(18)
                ^ (w[index - 15] >> 3);
            let s1 =
                w[index - 2].rotate_right(17) ^ w[index - 2].rotate_right(19) ^ (w[index - 2] >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;

        for (constant, scheduled) in Self::K.iter().zip(w.iter()) {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(*constant)
                .wrapping_add(*scheduled);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        for (slot, value) in self
            .state
            .iter_mut()
            .zip([a, b, c, d, e, f, g, h])
        {
            *slot = slot.wrapping_add(value);
        }
    }

    fn finish(mut self) -> String {
        let bit_length = self.length * 8;
        self.buffer.push(0x80);
        while self.buffer.len() % 64 != 56 {
            self.buffer.push(0);
        }
        self.buffer.extend_from_slice(&bit_length.to_be_bytes());

        let blocks: Vec<[u8; 64]> = self
            .buffer
            .chunks_exact(64)
            .map(|chunk| chunk.try_into().unwrap())
            .collect();
        for block in blocks {
            self.compress(&block);
        }

        self.state
            .iter()
            .map(|word| format!("{word:08x}"))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha256(input: &[u8]) -> String {
        let mut hasher = Sha256Writer::new();
        hasher.update(input);
        hasher.finish()
    }

    #[test]
    fn the_hasher_matches_published_vectors() {
        assert_eq!(
            sha256(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    /// Weights are hundreds of megabytes, so the hasher must handle input that
    /// spans many blocks and arrives in uneven chunks.
    #[test]
    fn hashing_is_independent_of_how_the_stream_is_chunked() {
        let data: Vec<u8> = (0..10_000u32).map(|n| (n % 251) as u8).collect();

        let mut chunked = Sha256Writer::new();
        for chunk in data.chunks(97) {
            chunked.update(chunk);
        }

        assert_eq!(chunked.finish(), sha256(&data));
    }

    #[test]
    fn a_download_reports_a_bounded_fraction() {
        for (received, total, expected) in [(0u64, 100u64, 0.0f32), (50, 100, 0.5), (100, 100, 1.0)]
        {
            let fraction = (received as f32 / total as f32).clamp(0.0, 1.0);
            assert!((fraction - expected).abs() < f32::EPSILON);
        }
    }
}
