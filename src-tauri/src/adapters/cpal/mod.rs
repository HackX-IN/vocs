/*!
 * SOURCE OF TRUTH KEYWORDS: cpal_adapter, CpalAudioSource, Resampler16k, OutputBridge,
 *   Passthrough, RateConverter
 * WHAT:  The AudioSource implementation backed by cpal / CoreAudio, and the output
 *        side Voice mode plays through (OutputBridge, the mic Passthrough).
 * WHERE: Constructed by adapters::build; the only implementation of the
 *        AudioSource port on macOS.
 */

pub mod convert;
pub mod output;
pub mod passthrough;
pub mod resample;
pub mod source;

pub use source::CpalAudioSource;
