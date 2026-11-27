//! Batch engines can recognize completed phrases while the microphone stays
//! open. Nothing is inserted until the whole dictation is processed. We cut
//! only inside silence, never guess how to merge overlapping spoken words.
//! Any overload, decode failure or excessive unbroken phrase falls back to
//! the original recording; an incomplete transcript is never delivered.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::Arc;

use crate::audio::SampleTap;

const RATE: usize = 16_000;
const FRAME: usize = RATE / 50;
const PAUSE: usize = RATE * 400 / 1000;
const MIN_PHRASE: usize = RATE * 3;
const MAX_PHRASE: usize = RATE * 25;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureOptions {
    pub model: String,
    pub language: Option<String>,
    pub prompt: Option<String>,
}

pub struct CaptureJob {
    pub provider_id: String,
    pub options: CaptureOptions,
    pub run: CaptureTranscription,
}

pub struct CaptureTranscription {
    sender: Option<SyncSender<Vec<i16>>>,
    result: Option<tokio::sync::oneshot::Receiver<Result<String, String>>>,
    cancelled: Arc<AtomicBool>,
}

impl CaptureTranscription {
    pub fn spawn(
        mut decode: impl FnMut(&[i16]) -> Result<String, String> + Send + 'static,
    ) -> Option<Self> {
        // At most 16 seconds queued, regardless of microphone callback size.
        let (sender, receiver) = sync_channel::<Vec<i16>>(256);
        let (result_tx, result) = tokio::sync::oneshot::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&cancelled);
        std::thread::Builder::new()
            .name("clide-capture-transcription".into())
            .spawn(move || {
                let result = (|| {
                    let mut segmenter = Phrases::default();
                    let mut transcript = Vec::new();
                    for samples in receiver {
                        if stop.load(Ordering::Acquire) {
                            return Err("capture transcription cancelled or overloaded".into());
                        }
                        for sample in samples {
                            if let Some(phrase) = segmenter.push(sample)? {
                                transcript.push(decode(&phrase)?);
                            }
                        }
                    }
                    if stop.load(Ordering::Acquire) {
                        return Err("capture transcription cancelled or overloaded".into());
                    }
                    if let Some(tail) = segmenter.finish() {
                        transcript.push(decode(&tail)?);
                    }
                    Ok(transcript.join(" "))
                })();
                let _ = result_tx.send(result);
            })
            .ok()?;
        Some(Self {
            sender: Some(sender),
            result: Some(result),
            cancelled,
        })
    }

    /// Only copies and try-sends on the audio callback: inference never runs
    /// there, and a full queue invalidates the result instead of dropping words.
    pub fn tap(&self) -> SampleTap {
        let sender = self.sender.as_ref().unwrap().clone();
        let cancelled = Arc::clone(&self.cancelled);
        Arc::new(move |samples| {
            for chunk in samples.chunks(1024) {
                if cancelled.load(Ordering::Acquire) {
                    break;
                }
                if sender.try_send(chunk.to_vec()).is_err() {
                    cancelled.store(true, Ordering::Release);
                    break;
                }
            }
        })
    }

    /// Detach the recorder's tap before calling this, closing the input stream.
    pub async fn finish(mut self) -> Result<String, String> {
        self.sender.take();
        self.result
            .take()
            .unwrap()
            .await
            .map_err(|_| "capture transcription worker stopped".to_string())?
    }
}

impl Drop for CaptureTranscription {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

#[derive(Default)]
struct Phrases {
    samples: Vec<i16>,
    quiet: usize,
    has_voice: bool,
}

impl Phrases {
    fn push(&mut self, sample: i16) -> Result<Option<Vec<i16>>, String> {
        self.samples.push(sample);
        if self.samples.len() % FRAME != 0 {
            return Ok(None);
        }
        let frame = &self.samples[self.samples.len() - FRAME..];
        let energy = frame
            .iter()
            .map(|&s| (s as f64 / 32768.0).powi(2))
            .sum::<f64>();
        // Deliberately conservative: only near-silence is a safe word boundary.
        if (energy / FRAME as f64).sqrt() <= 0.0015 {
            self.quiet += FRAME;
        } else {
            self.quiet = 0;
            self.has_voice = true;
        }
        if self.samples.len() >= MIN_PHRASE && self.quiet >= PAUSE {
            let phrase = self.finish();
            self.samples.clear();
            self.quiet = 0;
            self.has_voice = false;
            return Ok(phrase);
        }
        if self.samples.len() >= MAX_PHRASE {
            return Err("no safe phrase boundary; using the full recording".into());
        }
        Ok(None)
    }

    fn finish(&mut self) -> Option<Vec<i16>> {
        // Include a partial final frame in the speech check.
        if self.has_voice
            || self
                .samples
                .iter()
                .rev()
                .take(FRAME)
                .any(|s| s.unsigned_abs() > 100)
        {
            Some(std::mem::take(&mut self.samples))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrases_keep_every_sample_including_words_after_the_pause() {
        let mut phrases = Phrases::default();
        let mut first = vec![1200; RATE * 3];
        first.extend(vec![0; PAUSE]);
        let mut completed = None;
        for &sample in &first {
            completed = phrases.push(sample).unwrap().or(completed);
        }
        assert_eq!(completed.unwrap(), first);
        for _ in 0..RATE {
            assert!(phrases.push(2000).unwrap().is_none());
        }
        assert_eq!(phrases.finish().unwrap(), vec![2000; RATE]);
    }

    #[test]
    fn silence_is_not_sent_to_the_model() {
        let mut phrases = Phrases::default();
        for _ in 0..RATE * 10 {
            assert!(phrases.push(0).unwrap().is_none());
        }
        assert!(phrases.finish().is_none());
    }

    #[test]
    fn continuous_speech_falls_back_instead_of_cutting_a_word() {
        let mut phrases = Phrases::default();
        for _ in 0..MAX_PHRASE - 1 {
            assert!(phrases.push(1000).unwrap().is_none());
        }
        assert!(phrases.push(1000).is_err());
    }

    #[tokio::test]
    async fn finishing_drains_the_tail_and_preserves_cross_phrase_corrections() {
        let mut calls = 0;
        let run = CaptureTranscription::spawn(move |_| {
            calls += 1;
            Ok(if calls == 1 {
                "use red"
            } else {
                "scratch that use blue"
            }
            .into())
        })
        .unwrap();
        let tap = run.tap();
        let mut audio = vec![1000; RATE * 3];
        audio.extend(vec![0; PAUSE]);
        audio.extend(vec![1000; RATE]);
        tap(&audio);
        drop(tap);
        assert_eq!(run.finish().await.unwrap(), "use red scratch that use blue");
    }

    #[tokio::test]
    async fn worker_failure_never_returns_a_partial_transcript() {
        let run = CaptureTranscription::spawn(|_| Err("decode failed".into())).unwrap();
        let tap = run.tap();
        tap(&vec![1000; RATE]);
        drop(tap);
        assert!(run.finish().await.is_err());
    }

    #[tokio::test]
    async fn overload_invalidates_the_result_instead_of_losing_words() {
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let mut started_tx = Some(started_tx);
        let run = CaptureTranscription::spawn(move |_| {
            if let Some(tx) = started_tx.take() {
                let _ = tx.send(());
                release_rx.recv().unwrap();
            }
            Ok("partial".into())
        })
        .unwrap();
        let tap = run.tap();
        let mut phrase = vec![1000; RATE * 3];
        phrase.extend(vec![0; PAUSE]);
        tap(&phrase);
        started_rx.await.unwrap();
        // Inference is deliberately blocked, simulating a heavily loaded Mac.
        tap(&vec![1000; RATE * 20]);
        assert!(run.cancelled.load(Ordering::Acquire));
        release_tx.send(()).unwrap();
        drop(tap);
        assert!(run.finish().await.is_err());
    }

    #[tokio::test]
    async fn dropping_a_capture_cancels_queued_work() {
        let run = CaptureTranscription::spawn(|_| panic!("cancelled audio was decoded")).unwrap();
        let mut run = run;
        let result = run.result.take().unwrap();
        drop(run);
        assert!(result.await.unwrap().is_err());
    }
}
