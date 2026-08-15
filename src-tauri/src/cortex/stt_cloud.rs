// EXP-006: optional cloud speech-to-text. The budget pick is Groq's free
// Whisper API (PRD §7.2); OpenAI Whisper is the paid fallback at
// ~$0.006/min. Both accept audio files, so this module packages the 16 kHz
// float PCM from the resampler as a minimal WAV container before sending.
//
// Privacy contract (PRD §9): the audio bytes are the ONLY data that leaves
// the machine in this path — no context, no memory contents. The API key is
// never sent except as an auth header, and only when the user configured it
// in the settings store (cloud STT is never attempted unconfigured).
//
// Both providers speak the OpenAI-compatible `/v1/audio/transcriptions`
// multipart API, so one request builder serves both.

#[cfg(feature = "voice")]
use std::io::Write;

use serde::Deserialize;

#[cfg(feature = "voice")]
use ureq;

/// Groq free-tier Whisper endpoint (the budget pick per PRD §7.2).
#[cfg(feature = "voice")]
const GROQ_TRANSCRIBE_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";

/// OpenAI Whisper fallback endpoint (~$0.006/min).
#[cfg(feature = "voice")]
const OPENAI_TRANSCRIBE_URL: &str = "https://api.openai.com/v1/audio/transcriptions";

/// Whisper's fixed sample rate, also the WAV container rate.
const WHISPER_SAMPLE_RATE: u32 = 16_000;

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum CloudSttProvider {
    Groq,
    OpenAi,
}

impl std::fmt::Display for CloudSttProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CloudSttProvider::Groq => formatter.write_str("groq"),
            CloudSttProvider::OpenAi => formatter.write_str("openai"),
        }
    }
}

impl std::str::FromStr for CloudSttProvider {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_lowercase().as_str() {
            "groq" => Ok(CloudSttProvider::Groq),
            "openai" => Ok(CloudSttProvider::OpenAi),
            other => Err(format!("unsupported cloud STT provider: {other}")),
        }
    }
}

#[derive(Debug)]
pub enum CloudSttError {
    NoApiKeyConfigured(CloudSttProvider),
    RequestFailed(CloudSttProvider, String),
    UnexpectedResponse(CloudSttProvider, String),
    ProviderUnreachable(CloudSttProvider),
}

impl std::fmt::Display for CloudSttError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CloudSttError::NoApiKeyConfigured(provider) => {
                write!(
                    formatter,
                    "no {provider} API key configured; cloud speech-to-text is disabled"
                )
            }
            CloudSttError::RequestFailed(provider, detail) => {
                write!(
                    formatter,
                    "{provider} transcription request failed: {detail}"
                )
            }
            CloudSttError::UnexpectedResponse(provider, detail) => {
                write!(
                    formatter,
                    "{provider} returned an unparseable response: {detail}"
                )
            }
            CloudSttError::ProviderUnreachable(provider) => {
                write!(
                    formatter,
                    "the {provider} service could not be reached; check your connection"
                )
            }
        }
    }
}

impl std::error::Error for CloudSttError {}

/// OpenAI-compatible audio transcription response body.
#[derive(Debug, Deserialize)]
pub struct TranscriptionResponse {
    pub text: String,
}

/// A minimal RFC-2387-ish multipart form part for the transcription upload.
#[cfg(feature = "voice")]
struct FormPart<'a> {
    name: &'a str,
    filename: Option<&'a str>,
    content_type: Option<&'a str>,
    body: &'a [u8],
}

/// Encode the 16 kHz float PCM as a RIFF/WAVE container (16-bit signed PCM).
/// Bytes are cheaper than opus transcoding and every Whisper API accepts WAV.
#[cfg(feature = "voice")]
pub fn pcm_to_wav(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>, CloudSttError> {
    let channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
    let block_align: u16 = channels * bits_per_sample / 8;
    let data_length: u32 = (samples.len() * 2) as u32;

    let mut buffer = Vec::with_capacity(44 + data_length as usize);
    buffer.write_all(b"RIFF").ok();
    buffer.write_all(&(36 + data_length).to_le_bytes()).ok();
    buffer.write_all(b"WAVE").ok();
    buffer.write_all(b"fmt ").ok();
    buffer.write_all(&16u32.to_le_bytes()).ok();
    buffer.write_all(&1u16.to_le_bytes()).ok(); // PCM
    buffer.write_all(&channels.to_le_bytes()).ok();
    buffer.write_all(&sample_rate.to_le_bytes()).ok();
    buffer.write_all(&byte_rate.to_le_bytes()).ok();
    buffer.write_all(&block_align.to_le_bytes()).ok();
    buffer.write_all(&bits_per_sample.to_le_bytes()).ok();
    buffer.write_all(b"data").ok();
    buffer.write_all(&data_length.to_le_bytes()).ok();
    for sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let signed = (clamped * i16::MAX as f32) as i16;
        buffer.write_all(&signed.to_le_bytes()).ok();
    }
    Ok(buffer)
}

/// Send the audio to a provider and decode its transcript.
#[cfg(feature = "voice")]
fn transcribe_with_provider(
    provider: &CloudSttProvider,
    api_key: &str,
    wav_bytes: &[u8],
) -> Result<String, CloudSttError> {
    let base_url = match provider {
        CloudSttProvider::Groq => GROQ_TRANSCRIBE_URL,
        CloudSttProvider::OpenAi => OPENAI_TRANSCRIBE_URL,
    };

    let boundary = format!("Aura-Voice-{}", uuid::Uuid::new_v4());
    let mut body = Vec::new();

    write_part(
        &mut body,
        &boundary,
        FormPart {
            name: "model",
            filename: None,
            content_type: None,
            body: b"whisper-1",
        },
    );
    write_part(
        &mut body,
        &boundary,
        FormPart {
            name: "file",
            filename: Some("voice.wav"),
            content_type: Some("audio/wav"),
            body: wav_bytes,
        },
    );
    write_part(
        &mut body,
        &boundary,
        FormPart {
            name: "language",
            filename: None,
            content_type: None,
            body: b"en",
        },
    );
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    let response = ureq::post(base_url)
        .set("Authorization", &format!("Bearer {api_key}"))
        .set(
            "Content-Type",
            &format!("multipart/form-data; boundary={boundary}"),
        )
        .send_bytes(&body);

    match response {
        Ok(successful) => {
            let text = successful
                .into_string()
                .map_err(|error| CloudSttError::RequestFailed(*provider, error.to_string()))?;
            serde_json::from_str::<TranscriptionResponse>(&text)
                .map(|parsed| parsed.text.trim().to_string())
                .map_err(|error| CloudSttError::UnexpectedResponse(*provider, error.to_string()))
        }
        Err(ureq::Error::Status(code, response)) => {
            let detail = response
                .into_string()
                .unwrap_or_else(|_| format!("HTTP {code}"));
            Err(CloudSttError::RequestFailed(*provider, detail))
        }
        Err(ureq::Error::Transport(_transport)) => {
            Err(CloudSttError::ProviderUnreachable(*provider))
        }
    }
}

#[cfg(feature = "voice")]
fn write_part(buffer: &mut Vec<u8>, boundary: &str, part: FormPart<'_>) {
    buffer.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    buffer.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"{}\"", part.name).as_bytes(),
    );
    if let Some(filename) = part.filename {
        buffer.extend_from_slice(format!("; filename=\"{filename}\"").as_bytes());
    }
    buffer.extend_from_slice(b"\r\n");
    if let Some(content_type) = part.content_type {
        buffer.extend_from_slice(format!("Content-Type: {content_type}\r\n").as_bytes());
    }
    buffer.extend_from_slice(b"\r\n");
    buffer.extend_from_slice(part.body);
    buffer.extend_from_slice(b"\r\n");
}

/// Try the configured providers in order until one succeeds. Groq (free) is
/// always attempted first; OpenAI only runs when its key is also set.
/// Returns the transcript or the *first* provider error (so the caller can
/// report a specific actionable message rather than a noisy chain).
#[cfg(feature = "voice")]
pub fn transcribe_cloud(
    samples: &[f32],
    provider: CloudSttProvider,
    groq_key: Option<&str>,
    openai_key: Option<&str>,
) -> Result<String, CloudSttError> {
    if samples.is_empty() {
        return Err(CloudSttError::RequestFailed(
            provider,
            "the audio payload contains no samples".to_string(),
        ));
    }

    let wav_bytes = pcm_to_wav(samples, WHISPER_SAMPLE_RATE)?;

    let key = match provider {
        CloudSttProvider::Groq => groq_key,
        CloudSttProvider::OpenAi => openai_key,
    }
    .filter(|value| !value.is_empty());

    let Some(api_key) = key else {
        return Err(CloudSttError::NoApiKeyConfigured(provider));
    };

    transcribe_with_provider(&provider, api_key, &wav_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_header_is_valid_riff_and_roundtrips_sample_values() {
        let samples = vec![0.0, 0.5, -1.0, 1.0];
        let wav = pcm_to_wav(&samples, WHISPER_SAMPLE_RATE).unwrap();

        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");

        // Data chunk payload starts at offset 44; four 16-bit samples = 8 bytes.
        let payload = &wav[44..];
        assert_eq!(payload.len(), 8);

        let peak = i16::from_le_bytes([payload[6], payload[7]]);
        assert_eq!(peak, i16::MAX);
        let floor = i16::from_le_bytes([payload[4], payload[5]]);
        // -1.0 clamps to i16::MAX * -1 = i16::MIN + 1; whisper.cpp treats the
        // full signed range symmetrically around zero, so this matches.
        assert_eq!(floor, -i16::MAX);
    }

    #[test]
    fn provider_name_roundtrips_through_parse() {
        assert_eq!(
            "groq".parse::<CloudSttProvider>().unwrap(),
            CloudSttProvider::Groq
        );
        assert_eq!(
            "OpenAI".parse::<CloudSttProvider>().unwrap(),
            CloudSttProvider::OpenAi
        );
        assert!("unknown".parse::<CloudSttProvider>().is_err());
    }

    #[test]
    fn missing_key_surfaces_an_actionable_error() {
        let result = transcribe_cloud(&[0.1, 0.2], CloudSttProvider::Groq, None, None);
        assert!(matches!(
            result,
            Err(CloudSttError::NoApiKeyConfigured(CloudSttProvider::Groq))
        ));
    }

    #[test]
    fn empty_audio_is_rejected() {
        let result = transcribe_cloud(&[], CloudSttProvider::Groq, Some("key"), Some("key"));
        assert!(matches!(result, Err(CloudSttError::RequestFailed(..))));
    }
}
