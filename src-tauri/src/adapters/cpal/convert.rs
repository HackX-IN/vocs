/*!
 * SOURCE OF TRUTH KEYWORDS: RateConverter, convert_rate, mono_resample, flush,
 *   output_rate
 * WHAT:  Converts a mono f32 stream from one sample rate to another, in the
 *        fixed-size chunks the resampler wants.
 * WHY:   Voice mode has to play audio the Mac synthesised at 22.05 kHz, and to
 *        carry a 44.1 kHz microphone to a 48 kHz virtual device. resample.rs
 *        already holds the project's resampling discipline, but it is built for
 *        one job — anything to 16 kHz for Whisper — and bending it into a general
 *        converter would put output concerns into the capture path that the
 *        transcription accuracy depends on. This is the same cubic polynomial
 *        resampler with the target rate as a parameter, and the same rules:
 *        stateful (a fresh one per buffer clicks at every seam), fed exactly the
 *        chunk size it asks for, clamped to [-1, 1] because cubic interpolation
 *        overshoots, and absent entirely when the rates already match.
 *
 *        `flush` exists because speech ENDS: the last partial chunk would
 *        otherwise never be resampled and the final syllable would be missing.
 * WHERE: Used by adapters/cpal/output.rs (speech) and passthrough.rs (microphone).
 */

use audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Async, FixedAsync, PolynomialDegree, Resampler};

use crate::error::{AppError, AppResult, ErrorCode};

/// Frames per resampler call: ~21 ms at 48 kHz, the same trade as the capture side.
const CHUNK: usize = 1024;

pub struct RateConverter {
    resampler: Option<Async<f32>>,
    pending: Vec<f32>,
    scratch: Vec<f32>,
}

fn convert_error(err: impl std::fmt::Display) -> AppError {
    AppError::new(ErrorCode::AudioFormatUnsupported, "Vocs could not convert the audio's sample rate.")
        .with_detail(err.to_string())
}

impl RateConverter {
    pub fn new(from_rate: u32, to_rate: u32) -> AppResult<Self> {
        if from_rate == 0 || to_rate == 0 {
            return Err(convert_error("a sample rate of zero"));
        }
        let resampler = if from_rate == to_rate {
            None
        } else {
            Some(
                Async::<f32>::new_poly(
                    to_rate as f64 / from_rate as f64,
                    1.0,
                    PolynomialDegree::Cubic,
                    CHUNK,
                    1,
                    FixedAsync::Input,
                )
                .map_err(convert_error)?,
            )
        };
        Ok(Self { resampler, pending: Vec::with_capacity(CHUNK * 2), scratch: Vec::with_capacity(CHUNK) })
    }

    /// Whatever output is now complete. Empty is normal: the input did not yet
    /// fill a chunk.
    pub fn push(&mut self, mono: &[f32]) -> AppResult<Vec<f32>> {
        let Some(resampler) = self.resampler.as_mut() else {
            return Ok(mono.to_vec());
        };
        self.pending.extend_from_slice(mono);

        let mut out = Vec::new();
        loop {
            let needed = resampler.input_frames_next();
            if self.pending.len() < needed {
                break;
            }
            self.scratch.clear();
            self.scratch.extend_from_slice(&self.pending[..needed]);
            self.pending.drain(..needed);
            out.extend(process(resampler, &self.scratch)?);
        }
        Ok(out)
    }

    /// Resamples what is left, padded with silence to a full chunk. Call once,
    /// when the stream ends.
    pub fn flush(&mut self) -> AppResult<Vec<f32>> {
        let Some(resampler) = self.resampler.as_mut() else {
            return Ok(Vec::new());
        };
        if self.pending.is_empty() {
            return Ok(Vec::new());
        }
        let needed = resampler.input_frames_next();
        self.pending.resize(needed, 0.0);
        let padded = std::mem::take(&mut self.pending);
        process(resampler, &padded)
    }
}

fn process(resampler: &mut Async<f32>, frames: &[f32]) -> AppResult<Vec<f32>> {
    let input = InterleavedSlice::new(frames, 1, frames.len()).map_err(convert_error)?;
    let out = resampler.process(&input, None).map_err(convert_error)?;
    Ok(out.take_data().into_iter().map(|s| s.clamp(-1.0, 1.0)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(rate: u32, hz: f32, seconds: f32) -> Vec<f32> {
        (0..(rate as f32 * seconds) as usize)
            .map(|n| (2.0 * std::f32::consts::PI * hz * n as f32 / rate as f32).sin() * 0.5)
            .collect()
    }

    #[test]
    fn the_same_rate_is_passed_through_untouched() {
        let mut converter = RateConverter::new(48_000, 48_000).expect("builds");
        let input = sine(48_000, 440.0, 0.1);
        assert_eq!(converter.push(&input).expect("push"), input);
        assert!(converter.flush().expect("flush").is_empty());
    }

    #[test]
    fn a_second_of_audio_is_about_a_second_at_the_new_rate() {
        let mut converter = RateConverter::new(22_050, 48_000).expect("builds");
        let mut out = converter.push(&sine(22_050, 440.0, 1.0)).expect("push");
        out.extend(converter.flush().expect("flush"));
        // Within one chunk of exact: the tail is padded to a whole chunk.
        let expected = 48_000_i64;
        assert!((out.len() as i64 - expected).abs() < 2_500, "got {} frames", out.len());
    }

    #[test]
    fn the_tone_keeps_its_pitch() {
        let mut converter = RateConverter::new(22_050, 48_000).expect("builds");
        let out = converter.push(&sine(22_050, 440.0, 1.0)).expect("push");
        // Count rising zero crossings after the filter has settled: ~440 a second.
        let body = &out[2_000..out.len().min(2_000 + 48_000 / 2)];
        let crossings = body.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        let hz = crossings as f32 / (body.len() as f32 / 48_000.0);
        assert!((hz - 440.0).abs() < 15.0, "pitch drifted to {hz} Hz");
    }

    #[test]
    fn output_never_leaves_the_legal_range() {
        let mut converter = RateConverter::new(44_100, 48_000).expect("builds");
        let loud: Vec<f32> = (0..8_192).map(|n| if n % 2 == 0 { 1.0 } else { -1.0 }).collect();
        let out = converter.push(&loud).expect("push");
        assert!(out.iter().all(|s| (-1.0..=1.0).contains(s)));
    }

    #[test]
    fn a_zero_rate_is_refused() {
        assert!(RateConverter::new(0, 48_000).is_err());
        assert!(RateConverter::new(48_000, 0).is_err());
    }
}
