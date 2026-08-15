/**
 * EXP-006: unit tests for the push-to-talk audio path.
 *
 * The resampler is the only pure logic in `usePushToTalk` (the rest needs a
 * live browser MediaDevices context), so it is exported and covered here.
 * The surrounding hook is exercised manually in the friction-test loop and
 * by the Windows build smoke test.
 */
import { describe, expect, it } from "vitest";

import { resampleToWhisper } from "../pushToTalkAudio";

const WHISPER_SAMPLE_RATE = 16_000;

describe("resampleToWhisper", () => {
  it("passes 16 kHz samples through unchanged", () => {
    const source = new Float32Array([0.1, 0.5, 0.9]);
    expect(resampleToWhisper(source, WHISPER_SAMPLE_RATE)).toEqual(source);
  });

  it("downsamples 48 kHz to exactly ceil(n * ratio) samples", () => {
    const source = new Float32Array(1600);
    for (let index = 0; index < source.length; index += 1) {
      source[index] = index / 1600;
    }
    const output = resampleToWhisper(source, 48_000);
    expect(output.length).toBe(534);
    // Endpoints survive linear interpolation.
    expect(output[0]).toBeCloseTo(0, 1);
    expect(output[output.length - 1]).toBeCloseTo(1, 1);
  });

  it("upsamples 8 kHz and handles empty input", () => {
    const source = new Float32Array(160);
    for (let index = 0; index < source.length; index += 1) {
      source[index] = index / 160;
    }
    expect(resampleToWhisper(source, 8_000).length).toBe(320);
    expect(resampleToWhisper(new Float32Array(0), 44_100).length).toBe(0);
    expect(resampleToWhisper(new Float32Array([0.5]), 0).length).toBe(0);
  });
});
