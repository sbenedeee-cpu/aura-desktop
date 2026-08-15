// EXP-006: on-device speech-to-text via whisper-rs (whisper.cpp).
//
// The model is intentionally NOT bundled: it is downloaded once at first use
// (PRD risk §12) into the app data directory and re-used on every subsequent
// session. The base model (~145 MB) is the floor — small enough to ship the
// first download in a reasonable time, large enough to hit the ≥90 %
// transcript fidelity target on natural dictation.
//
// whisper.cpp works on normalized 32-bit float PCM at 16 kHz mono. The webview
// resampler hands us f32 samples at whatever rate the microphone gave us, so
// this module owns both the resampling and the whisper invocation.

#[cfg(feature = "voice")]
use std::path::{Path, PathBuf};

#[cfg(feature = "voice")]
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Official whisper.cpp model repository.
#[cfg(feature = "voice")]
const WHISPER_MODEL_HOST: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// The on-device floor model. Base is the documented v1 floor (PRD §7.2);
/// users with 16 GB+ RAM can later swap to `ggml-small.bin` via settings.
#[cfg(feature = "voice")]
const DEFAULT_MODEL_FILE: &str = "ggml-base.bin";

/// Maximum accepted recording duration. Recordings longer than this are
/// rejected rather than silently truncated, keeping push-to-talk sessions
/// bounded (and protecting offline inference time on slow CPUs).
#[cfg(feature = "voice")]
pub const MAX_RECORDING_SECONDS: f32 = 60.0;

/// Whisper.cpp's fixed sample rate.
const WHISPER_SAMPLE_RATE: u32 = 16_000;

#[derive(Debug)]
pub enum LocalSttError {
    ModelDownload(String),
    TooLong(f32),
    EmptyAudio,
    Transcription(String),
}

impl std::fmt::Display for LocalSttError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LocalSttError::ModelDownload(detail) => {
                write!(formatter, "could not download the whisper model: {detail}")
            }
            LocalSttError::TooLong(limit) => {
                write!(
                    formatter,
                    "recording exceeded the {:.0} second push-to-talk limit",
                    limit
                )
            }
            LocalSttError::EmptyAudio => {
                formatter.write_str("the audio payload contains no samples")
            }
            LocalSttError::Transcription(detail) => {
                write!(formatter, "local transcription failed: {detail}")
            }
        }
    }
}

impl std::error::Error for LocalSttError {}

/// Where the downloaded model lives relative to the app data directory.
#[cfg(feature = "voice")]
pub fn model_path(data_dir: &Path) -> PathBuf {
    data_dir.join("whisper").join(DEFAULT_MODEL_FILE)
}

/// `true` when a usable model is already on disk.
#[cfg(feature = "voice")]
pub fn is_model_available(data_dir: &Path) -> bool {
    let path = model_path(data_dir);
    path.exists()
        && path
            .metadata()
            .map(|metadata| metadata.len() > 1024)
            .unwrap_or(false)
}

/// Fetch the base whisper model once, storing it under
/// `{data_dir}/whisper/ggml-base.bin`. Idempotent: a present file short-circuits.
#[cfg(feature = "voice")]
pub fn ensure_model_downloaded(data_dir: &Path) -> Result<PathBuf, LocalSttError> {
    let target = model_path(data_dir);
    if target.exists() {
        if target
            .metadata()
            .map(|metadata| metadata.len() > 1024)
            .unwrap_or(false)
        {
            return Ok(target);
        }
        // A stub file survived a previous interrupted download; start over.
        let _ = std::fs::remove_file(&target);
    }

    std::fs::create_dir_all(target.parent().unwrap())
        .map_err(|error| LocalSttError::ModelDownload(error.to_string()))?;

    // Write to a staging file first so a killed download never leaves a
    // half-named model that looks valid to `is_model_available`.
    let staging = target.with_extension("bin.downloading");
    let url = format!("{WHISPER_MODEL_HOST}/{DEFAULT_MODEL_FILE}");

    let response = ureq::get(&url)
        .call()
        .map_err(|error| LocalSttError::ModelDownload(error.to_string()))?;

    let body = response
        .into_string()
        .map_err(|error| LocalSttError::ModelDownload(error.to_string()))?
        .into_bytes();

    std::fs::write(&staging, &body)
        .map_err(|error| LocalSttError::ModelDownload(error.to_string()))?;

    std::fs::rename(&staging, &target)
        .map_err(|error| LocalSttError::ModelDownload(error.to_string()))?;

    Ok(target)
}

/// Convert raw microphone samples at `source_rate` into the 16 kHz float
/// stream whisper.cpp expects. A linear-interpolation resampler is exact
/// enough for 16 kHz output and avoids a heavy DSP dependency; sample rates
/// at or above 16 kHz are downsampled, lower rates upsampled.
pub fn resample_to_whisper(source: &[f32], source_rate: u32) -> Vec<f32> {
    if source_rate == WHISPER_SAMPLE_RATE {
        return source.to_vec();
    }
    if source.is_empty() || source_rate == 0 {
        return Vec::new();
    }

    let ratio = WHISPER_SAMPLE_RATE as f64 / source_rate as f64;
    let out_len = (source.len() as f64 * ratio).ceil() as usize;
    let mut output = Vec::with_capacity(out_len);
    for index in 0..out_len {
        let source_index = (index as f64) / ratio;
        let lower = source_index.floor() as usize;
        let upper = (lower + 1).min(source.len().saturating_sub(1));
        let fraction = source_index - lower as f64;
        let value = source[lower] as f64 * (1.0 - fraction) + source[upper] as f64 * fraction;
        output.push(value as f32);
    }
    output
}

/// Transcribe 16 kHz float PCM on-device. The whisper context is created
/// per-call by design: EXP-005 keeps the overlay sessions short, and holding
/// the context alive across a cold app start is EXP-007's caching concern.
#[cfg(feature = "voice")]
pub fn transcribe_pcm(model: &Path, samples: &[f32]) -> Result<String, LocalSttError> {
    if samples.is_empty() {
        return Err(LocalSttError::EmptyAudio);
    }

    let seconds = samples.len() as f32 / WHISPER_SAMPLE_RATE as f32;
    if seconds > MAX_RECORDING_SECONDS {
        return Err(LocalSttError::TooLong(MAX_RECORDING_SECONDS));
    }

    let context = WhisperContext::new_with_params(model, WhisperContextParameters::new())
        .map_err(|error| LocalSttError::Transcription(format!("model load failed: {error}")))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    // English-only keeps the base model accurate for the v1 user's dictation
    // without paying the multi-language quality tax.
    params.set_language(Some("en"));
    params.set_single_segment(true);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_no_timestamps(true);

    let mut state = context
        .create_state()
        .map_err(|error| LocalSttError::Transcription(error.to_string()))?;

    let owned = samples.to_vec();
    state
        .full(params, &owned)
        .map_err(|error| LocalSttError::Transcription(error.to_string()))?;

    let mut transcript = String::new();
    let segment_count = state.full_n_segments();
    for segment_index in 0..segment_count {
        let Some(segment) = state.get_segment(segment_index) else {
            continue;
        };
        let text = segment
            .to_str()
            .map_err(|error| LocalSttError::Transcription(error.to_string()))?;
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            if !transcript.is_empty() {
                transcript.push(' ');
            }
            transcript.push_str(trimmed);
        }
    }

    Ok(transcript)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_rate_is_a_no_op() {
        let source = vec![0.1, 0.5, 0.9];
        assert_eq!(resample_to_whisper(&source, WHISPER_SAMPLE_RATE), source);
    }

    #[test]
    fn resample_downsamples_to_16k() {
        // 48 kHz, 1600 samples = 33.333 ms; expect ceil to 534 output samples.
        let source: Vec<f32> = (0..1600).map(|sample| (sample as f32) / 1600.0).collect();
        let output = resample_to_whisper(&source, 48_000);
        assert_eq!(output.len(), 534);
        // Endpoints survive interpolation unchanged.
        assert!((output.first().copied().unwrap_or(0.0) - 0.0).abs() < 0.01);
        assert!((output.last().copied().unwrap_or(0.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn resample_handles_upsampling_and_empty_input() {
        let source: Vec<f32> = (0..160).map(|sample| (sample as f32) / 160.0).collect();
        assert_eq!(resample_to_whisper(&source, 8_000).len(), 320);
        assert!(resample_to_whisper(&[], 44_100).is_empty());
    }

    #[test]
    fn max_recording_limit_is_sane() {
        assert!((0.5..=120.0).contains(&MAX_RECORDING_SECONDS));
    }

    #[test]
    fn error_display_is_human_readable() {
        let message = format!("{}", LocalSttError::TooLong(60.0));
        assert!(message.contains("push-to-talk limit"));
    }
}
