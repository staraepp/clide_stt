//! Downmix + decimation to the one format Clide sends to providers.
//!
//! Capture devices hand us whatever they like (48 kHz stereo f32 is typical).
//! Whisper-family models work at 16 kHz mono, so converting here means the
//! provider layer never has to transcode and we can write a WAV that Groq
//! accepts as-is.

/// Everything downstream of capture assumes this rate.
pub const TARGET_SAMPLE_RATE: u32 = 16_000;

/// Streaming interleaved-multichannel -> mono 16 kHz converter.
///
/// Averaging every input sample that falls inside an output window (rather
/// than point-sampling) gives cheap anti-aliasing, which matters going from
/// 48 kHz to 16 kHz: naive decimation folds high-frequency energy back down
/// into the speech band.
pub struct MonoDownsampler {
    channels: usize,
    /// Input frames consumed per output sample.
    ratio: f64,
    position: f64,
    sum: f32,
    count: u32,
}

impl MonoDownsampler {
    pub fn new(source_rate: u32, channels: u16) -> Self {
        let channels = channels.max(1) as usize;
        let source_rate = source_rate.max(1);
        Self {
            channels,
            ratio: source_rate as f64 / TARGET_SAMPLE_RATE as f64,
            position: 0.0,
            sum: 0.0,
            count: 0,
        }
    }

    /// Feed one interleaved buffer straight from the capture callback and
    /// append the resulting 16 kHz mono samples to `out`.
    pub fn push(&mut self, interleaved: &[f32], out: &mut Vec<i16>) {
        for frame in interleaved.chunks_exact(self.channels) {
            let mono = frame.iter().sum::<f32>() / self.channels as f32;
            self.sum += mono;
            self.count += 1;
            self.position += 1.0;

            while self.position >= self.ratio {
                self.position -= self.ratio;
                if self.count > 0 {
                    out.push(to_i16(self.sum / self.count as f32));
                    self.sum = 0.0;
                    self.count = 0;
                }
            }
        }
    }

    /// Emit whatever partial window is left. Called once when capture stops so
    /// the final few milliseconds of speech are not dropped.
    pub fn flush(&mut self, out: &mut Vec<i16>) {
        if self.count > 0 {
            out.push(to_i16(self.sum / self.count as f32));
            self.sum = 0.0;
            self.count = 0;
        }
    }
}

fn to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

/// Root-mean-square level of an interleaved buffer, in 0.0..=1.0.
///
/// Computed in the capture callback so the HUD waveform reacts to the real
/// microphone rather than to a decorative animation.
pub fn rms_level(interleaved: &[f32]) -> f32 {
    if interleaved.is_empty() {
        return 0.0;
    }
    let sum_squares: f32 = interleaved.iter().map(|s| s * s).sum();
    (sum_squares / interleaved.len() as f32).sqrt().clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_48k_becomes_mono_16k_at_one_third_the_length() {
        let mut d = MonoDownsampler::new(48_000, 2);
        let mut out = Vec::new();
        // 4800 stereo frames = 100 ms at 48 kHz -> expect ~1600 samples.
        let input = vec![0.5f32; 4800 * 2];
        d.push(&input, &mut out);
        d.flush(&mut out);
        assert!((out.len() as i32 - 1600).abs() <= 1, "got {}", out.len());
    }

    #[test]
    fn channels_are_averaged_not_concatenated() {
        let mut d = MonoDownsampler::new(16_000, 2);
        let mut out = Vec::new();
        // Left at +1.0, right at -1.0 must average to silence.
        d.push(&[1.0, -1.0, 1.0, -1.0], &mut out);
        d.flush(&mut out);
        assert!(out.iter().all(|s| s.abs() < 8), "{out:?}");
    }

    #[test]
    fn a_matching_rate_passes_through_sample_for_sample() {
        let mut d = MonoDownsampler::new(16_000, 1);
        let mut out = Vec::new();
        d.push(&[1.0, 1.0, 1.0, 1.0], &mut out);
        d.flush(&mut out);
        assert_eq!(out.len(), 4);
        assert!(out.iter().all(|s| *s > 32_000));
    }

    #[test]
    fn streaming_in_chunks_matches_one_big_buffer() {
        let input: Vec<f32> = (0..4800).map(|i| (i as f32 / 90.0).sin()).collect();

        let mut whole = Vec::new();
        let mut d = MonoDownsampler::new(48_000, 1);
        d.push(&input, &mut whole);
        d.flush(&mut whole);

        let mut chunked = Vec::new();
        let mut d = MonoDownsampler::new(48_000, 1);
        for chunk in input.chunks(157) {
            d.push(chunk, &mut chunked);
        }
        d.flush(&mut chunked);

        assert_eq!(whole, chunked);
    }

    #[test]
    fn clipping_never_wraps_around() {
        let mut d = MonoDownsampler::new(16_000, 1);
        let mut out = Vec::new();
        d.push(&[9.0, -9.0], &mut out);
        d.flush(&mut out);
        assert!(out.iter().all(|s| s.abs() >= 32_000));
    }

    #[test]
    fn silence_and_full_scale_bracket_the_level_range() {
        assert_eq!(rms_level(&[0.0; 64]), 0.0);
        assert!((rms_level(&[1.0; 64]) - 1.0).abs() < 1e-6);
        assert!(rms_level(&[]) == 0.0);
    }
}
