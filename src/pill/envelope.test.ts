import { describe, expect, it } from "vitest";
import { smoothLevel } from "./envelope";

describe("waveform envelope", () => {
  it("behaves the same at different frame rates", () => {
    let fast = 0;
    let slow = 0;
    for (let i = 0; i < 60; i++) fast = smoothLevel(fast, 0.8, 1000 / 60);
    for (let i = 0; i < 30; i++) slow = smoothLevel(slow, 0.8, 1000 / 30);
    expect(fast).toBeCloseTo(slow, 8);
  });

  it("responds to speech faster than it falls back to silence", () => {
    expect(smoothLevel(0, 1, 50)).toBeGreaterThan(1 - smoothLevel(1, 0, 50));
    expect(smoothLevel(0, 1, 16)).toBeLessThan(0.25);
    expect(smoothLevel(0.8, 0, 1000)).toBeLessThan(0.01);
  });

  it("rejects invalid input and clamps microphone values", () => {
    expect(smoothLevel(0, NaN, 16)).toBe(0);
    expect(smoothLevel(0, -1, 16)).toBe(0);
    expect(smoothLevel(0, 100, 16)).toBeLessThan(1);
    expect(smoothLevel(0.4, 1, -10)).toBe(0.4);
  });
});
