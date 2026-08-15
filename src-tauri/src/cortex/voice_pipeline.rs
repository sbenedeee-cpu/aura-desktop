// EXP-006: the public dispatch surface of the Voice Pipeline.
//
// STT tier resolution follows PRD §7.2 priority order:
//   1. configured cloud provider + reachable API  → cloud (Groq first, OpenAI fallback)
//   2. otherwise                                 → local whisper-rs (model downloaded once)
// The overlay only ever sees `TranscriptionResult`; it does not know which
// tier ran, so the renderer has no cloud-specific behavior to leak.

use serde::{Deserialize, Serialize};

use super::stt_cloud::{self, CloudSttError, CloudSttProvider};
use super::stt_local::{self, LocalSttError};

/// What arrived with the recording from the webview. The webview resamples
/// the microphone stream to 16 kHz internally (see Overlay.tsx), so the
/// samples arrive already at the rate whisper.cpp requires; this pipeline
/// resamples defensively in case a third-party recorder overrides that.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionRequest {
    /// Normalized float PCM samples from the webview resampler.
    pub samples: Vec<f32>,
    /// Sample rate of the incoming samples (16000 expected).
    pub sample_rate: u32,
    /// Cloud mode flag set by the settings store; `auto` resolves at runtime.
    pub prefer_cloud: bool,
}

/// The single value the overlay renders as the dictated command.
#[derive(Debug, Serialize)]
pub struct TranscriptionResult {
    pub transcript: String,
    pub source: String,
}

#[derive(Debug)]
pub enum PipelineError {
    Local(LocalSttError),
    Cloud(CloudSttError),
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineError::Local(error) => write!(formatter, "{error}"),
            PipelineError::Cloud(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for PipelineError {}

impl From<LocalSttError> for PipelineError {
    fn from(error: LocalSttError) -> Self {
        PipelineError::Local(error)
    }
}

impl From<CloudSttError> for PipelineError {
    fn from(error: CloudSttError) -> Self {
        PipelineError::Cloud(error)
    }
}

/// Transcribe a push-to-talk recording. Cloud runs first when the user opted
/// into it AND a key exists; any cloud failure degrades to the local path so
/// the offline-capable core law (PRD §4.3) always holds.
#[cfg(feature = "voice")]
pub fn transcribe(
    request: TranscriptionRequest,
    data_dir: &std::path::Path,
    groq_key: Option<&str>,
    openai_key: Option<&str>,
) -> Result<TranscriptionResult, PipelineError> {
    let samples = stt_local::resample_to_whisper(&request.samples, request.sample_rate);

    if request.prefer_cloud {
        // Cloud attempt: Groq primary, OpenAI fallback. A cloud failure is
        // never terminal — it degrades to the local whisper-rs path below.
        for provider in [CloudSttProvider::Groq, CloudSttProvider::OpenAi] {
            match stt_cloud::transcribe_cloud(&samples, provider, groq_key, openai_key) {
                Ok(transcript) => {
                    return Ok(TranscriptionResult {
                        transcript,
                        source: format!("cloud:{provider}"),
                    });
                }
                Err(CloudSttError::NoApiKeyConfigured(_)) => {
                    // No key for this provider: try the next one, then local.
                    continue;
                }
                Err(_) => {
                    // Transient network or API error: fall through to local.
                    break;
                }
            }
        }
    }

    // Local path: download the model once, then transcribe on-device.
    stt_local::ensure_model_downloaded(data_dir)?;
    let model = stt_local::model_path(data_dir);
    let transcript = stt_local::transcribe_pcm(&model, &samples)?;
    Ok(TranscriptionResult {
        transcript,
        source: "local:whisper-rs".to_string(),
    })
}

/// The non-voice compilation path still exists for documentation builds; it
/// always refuses with a clear error rather than silently succeeding.
#[cfg(not(feature = "voice"))]
pub fn transcribe(
    _request: TranscriptionRequest,
    _data_dir: &std::path::Path,
    _groq_key: Option<&str>,
    _openai_key: Option<&str>,
) -> Result<TranscriptionResult, PipelineError> {
    Err(PipelineError::Offline(
        "Aura was built without voice support; enable the `voice` feature to transcribe audio."
            .to_string(),
    ))
}
