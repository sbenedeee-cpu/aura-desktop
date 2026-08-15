# ADR-008: Push-to-Talk Voice Pipeline

**Status:** Accepted
**Supersedes:** N/A
**Follows:** ADR-007 (Jarvis pivot, Neural Cortex)

## Context

EXP-005 proved the Neural Cortex summon loop (Alt+Space overlay with an echo brain). The next input channel is voice, and the PRD is explicit about the privacy contract: no always-listening audio, push-to-talk only. The assistant must transcribe dictation into the command line without leaving the overlay, and transcription must remain usable offline because the core law (PRD §4.3) requires the local store and local paths to function without network access.

Three transcription strategies were considered:

| Option | Cost | Offline | Latency | Accuracy floor |
|---|---|---|---|---|
| A. Cloud Whisper API only (Groq/OpenAI) | Free tier / ~$0.006/min | No | 0.5–1.5 s | Highest |
| B. In-browser Web Speech API | Free | Partially | Low | Poor for dictation, no transcript control |
| C. Local whisper.cpp (whisper-rs) with optional cloud boost | ~145 MB model download | Yes | 2–6 s on CPU | ≥90 % on natural dictation |

## Decision

We implement option C with a tiered dispatcher. The webview captures audio exclusively during an explicit press-and-hold session (Space while the input is focused, or a mic click), resamples it to 16 kHz mono float PCM, and sends the bytes to the Rust core via a single `transcribe_audio` command. The Rust pipeline downloads the base whisper.cpp model once at first use (`{app_data}/whisper/ggml-base.bin`), transcribes on-device, and never persists audio to disk. When the user configures a Groq key (EXP-007 settings store), Groq's free Whisper API becomes the primary tier with OpenAI as the paid fallback; any cloud failure degrades to the local path so offline behavior always holds.

The model is deliberately not bundled with the binary: a 145 MB download at first use keeps the installer small and honors the PRD risk log (§12), and a staging-file download strategy makes interrupted downloads non-poisonous.

## Consequences

The overlay shows a recording state (hold Space / mic click), a transcribing state, and renders transcript errors in the reply area. Windows TTS (SAPI) is deferred to EXP-007: the voice pipeline is input-first, and the same buffer handoff model (`Float32Array → Rust`) extends cleanly to TTS. Whisper context creation is per-call by design — EXP-007 owns long-lived context caching. CI now requires `libclang-dev` on Linux because `whisper-rs` builds whisper.cpp natively; the Windows build gate is unaffected.

## Open questions

The base model is the floor; a `ggml-small.bin` upgrade path (for 16 GB+ RAM machines) is a documented follow-up, not a decision to be made now.
