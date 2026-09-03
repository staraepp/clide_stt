//! Decoding a recording for a local engine.
//!
//! Both local engines want 16 kHz mono `f32`, which is exactly what Clide
//! captures — so this is a decode, never a resample.

use crate::providers::error::ProviderError;

/// whisper.cpp wants 16 kHz mono `f32`. Clide already captures exactly that, so
/// this is a decode rather than a resample.
pub fn read_wav_as_mono_f32(path: &std::path::Path) -> Result<Vec<f32>, ProviderError> {
    let mut reader = hound::WavReader::open(path)
        .map_err(|e| ProviderError::AudioUnreadable(e.to_string()))?;
    let spec = reader.spec();

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| ProviderError::AudioUnreadable(e.to_string()))?,
        hound::SampleFormat::Int => reader
            .samples::<i16>()
            .map(|sample| sample.map(|value| value as f32 / i16::MAX as f32))
            .collect::<Result<_, _>>()
            .map_err(|e| ProviderError::AudioUnreadable(e.to_string()))?,
    };

    if spec.channels <= 1 {
        return Ok(samples);
    }

    // Downmix defensively; the capture path should never produce this.
    let channels = spec.channels as usize;
    Ok(samples
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect())
}

