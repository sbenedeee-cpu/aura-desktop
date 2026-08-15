// EXP-006: the Voice Pipeline — Neural Cortex input path.
//
// Push-to-talk only (D4): audio only exists for the duration of an explicit
// recording session started in the overlay. Recordings are resampled to the
// 16 kHz mono float stream whisper.cpp requires, then transcribed locally
// (whisper-rs) or, when the user has configured a cloud key, through the
// Groq Whisper API with OpenAI as the fallback. The local store remains the
// only repository of long-term data; no audio is ever persisted.
//
// Module layout:
//   stt_local   — on-device whisper-rs transcription (lazy model download)
//   stt_cloud   — optional cloud Whisper transcription (Groq → OpenAI)
//   voice_pipeline — public dispatcher that resolves the STT tier

pub mod stt_cloud;
pub mod stt_local;
pub mod voice_pipeline;
