/**
 * EXP-006: pure audio helpers for push-to-talk capture.
 *
 * Kept separate from `usePushToTalk` so the resampler can be unit-tested
 * without a browser MediaDevices context.
 */

/** The sample rate whisper.cpp requires. */
export const WHISPER_SAMPLE_RATE = 16_000;

/**
 * Resample native-rate mono samples to the 16 kHz stream whisper.cpp
 * expects. Linear interpolation is exact enough at this rate and avoids a
 * heavy DSP dependency in the renderer.
 */
export function resampleToWhisper(samples: Float32Array, sampleRate: number): Float32Array {
  if (sampleRate === WHISPER_SAMPLE_RATE) {
    return samples;
  }
  if (samples.length === 0 || sampleRate <= 0) {
    return new Float32Array(0);
  }
  const ratio = WHISPER_SAMPLE_RATE / sampleRate;
  const outputLength = Math.ceil(samples.length * ratio);
  const output = new Float32Array(outputLength);
  for (let outputIndex = 0; outputIndex < outputLength; outputIndex += 1) {
    const sourceIndex = outputIndex / ratio;
    const lower = Math.floor(sourceIndex);
    const upper = Math.min(lower + 1, samples.length - 1);
    const fraction = sourceIndex - lower;
    output[outputIndex] = samples[lower]! * (1 - fraction) + samples[upper]! * fraction;
  }
  return output;
}
