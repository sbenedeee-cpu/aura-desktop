/**
 * EXP-006: push-to-talk capture hook for the Neural Cortex overlay.
 *
 * Privacy contract (PRD §9): recording ONLY exists while the user holds the
 * hold key. Audio lives in memory for the duration of the session and is
 * never written to disk — the webview hands raw float samples to the Rust
 * core, which transcribes and discards the bytes.
 *
 * Audio path:
 *   microphone (MediaRecorder, webm/opus)
 *   → decode via OfflineAudioContext → Float32Array at native rate
 *   → linear interpolation to 16 kHz mono float PCM (whisper.cpp input)
 *   → invoke `transcribe_audio` (Rust, EXP-006) → transcript into input field
 *
 * `preferCloud: false` is hardcoded for v1: the first release is strictly
 * local-first (PRD §7.2); cloud STT wiring comes with the settings store in
 * EXP-007.
 */
import { useCallback, useRef, useState } from "react";

import { resampleToWhisper } from "./pushToTalkAudio";

/** The sample rate whisper.cpp requires. */
const WHISPER_SAMPLE_RATE = 16_000;

/** v1 contract: strictly local transcription. */
const PREFER_CLOUD = false;

export type VoiceStatus = "idle" | "listening" | "transcribing" | "unavailable";

export function usePushToTalk() {
  const [status, setStatus] = useState<VoiceStatus>("idle");
  const [error, setError] = useState<string>("");
  const recorderRef = useRef<MediaRecorder | null>(null);
  const streamRef = useRef<MediaStream | null>(null);

  const stopListening = useCallback(() => {
    const recorder = recorderRef.current;
    const stream = streamRef.current;
    recorderRef.current = null;
    streamRef.current = null;
    stream?.getTracks().forEach((track) => track.stop());
    if (recorder && recorder.state !== "inactive") recorder.stop();
  }, []);

  /** Decode a webm/opus blob into a Float32Array at the browser's native rate. */
  const decodeToFloat = useCallback(
    async (blob: Blob): Promise<{ samples: Float32Array; sampleRate: number }> => {
      const buffer = await blob.arrayBuffer();
      const decoded = await new AudioContext().decodeAudioData(buffer.slice(0));
      // Opus decodes to stereo; mix to mono before resampling so the channel
      // count never surprises the resampler below.
      let mono: Float32Array;
      if (decoded.numberOfChannels === 1) {
        mono = decoded.getChannelData(0);
      } else {
        mono = new Float32Array(decoded.length);
        const channels = Array.from({ length: decoded.numberOfChannels }, (_, channelIndex) =>
          decoded.getChannelData(channelIndex),
        );
        for (let sampleIndex = 0; sampleIndex < decoded.length; sampleIndex += 1) {
          let sum = 0;
          for (const channel of channels) {
            sum += channel[sampleIndex] ?? 0;
          }
          mono[sampleIndex] = sum / channels.length;
        }
      }
      return { samples: mono, sampleRate: decoded.sampleRate };
    },
    [],
  );

  const transcribe = useCallback(
    async (samples: Float32Array, sampleRate: number, onTranscript: (text: string) => void) => {
      const { invoke } = await import("@tauri-apps/api/core");
      try {
        const result = (await invoke<{ transcript: string; source: string }>("transcribe_audio", {
          request: {
            samples: Array.from(samples),
            sampleRate,
            preferCloud: PREFER_CLOUD,
          },
        })) as { transcript: string; source: string };
        onTranscript(result.transcript.trim());
      } catch (invocationError) {
        const message =
          invocationError instanceof Error
            ? invocationError.message
            : "Aura could not transcribe that recording.";
        setError(message);
        // Surface transcript-stage failures in the reply area too, since the
        // overlay gives the user no other error affordance.
        onTranscript("");
      }
    },
    [],
  );

  /**
   * Request the microphone and begin capturing. Returns an error string on
   * failure (no throw) so the overlay can render an actionable message.
   */
  const startListening = useCallback(async (): Promise<string | null> => {
    setError("");
    if (!navigator.mediaDevices?.getUserMedia) {
      setStatus("unavailable");
      return "Voice input is not available in this environment.";
    }
    try {
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: { echoCancellation: true, noiseSuppression: true },
      });
      streamRef.current = stream;

      const recorder = new MediaRecorder(stream, { mimeType: "audio/webm;codecs=opus" });
      recorderRef.current = recorder;
      const chunks: Blob[] = [];
      recorder.addEventListener("dataavailable", (event) => {
        if (event.data.size > 0) chunks.push(event.data);
      });
      recorder.addEventListener("stop", async () => {
        setStatus("transcribing");
        try {
          const blob = new Blob(chunks, { type: "audio/webm" });
          if (blob.size < 512) {
            // Too short to carry speech; treat as a misfire, not a failure.
            setStatus("idle");
            return;
          }
          const { samples, sampleRate } = await decodeToFloat(blob);
          const whisperSamples = resampleToWhisper(samples, sampleRate);
          await transcribe(whisperSamples, WHISPER_SAMPLE_RATE, (text) => {
            setStatus("idle");
            if (text) {
              const event = new CustomEvent("aura-voice-transcript", { detail: text });
              window.dispatchEvent(event);
            }
          });
        } catch (decodingError) {
          setError(
            decodingError instanceof Error
              ? decodingError.message
              : "Aura could not process that recording.",
          );
          setStatus("idle");
        }
      });

      recorder.start();
      setStatus("listening");
      return null;
    } catch (captureError) {
      const name = (captureError as { name?: string }).name ?? "";
      if (name === "NotAllowedError" || name === "SecurityError") {
        setStatus("unavailable");
        return "Microphone access was denied. Allow it in your OS settings and try again.";
      }
      setStatus("unavailable");
      return "Aura could not reach the microphone. Is another app holding it?";
    }
  }, [decodeToFloat, transcribe]);

  return { error, startListening, status, stopListening };
}
