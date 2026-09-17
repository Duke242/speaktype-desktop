import { describe, expect, it } from "vitest";
import {
  accuracyTier,
  countWords,
  formatBytes,
  formatClock,
  formatDuration,
  formatLanguages,
  formatModelSize,
  formatRelative,
  hotkeyParts,
  speedTier,
  startOfDay,
} from "./format";

describe("durations and sizes", () => {
  it("formats clocks and durations", () => {
    expect(formatClock(0)).toBe("0:00");
    expect(formatClock(75.9)).toBe("1:15");
    expect(formatClock(-3)).toBe("0:00");
    expect(formatDuration(12)).toBe("12s");
    expect(formatDuration(95)).toBe("1m 35s");
    expect(formatDuration(4500)).toBe("1h 15m");
  });

  it("formats model and byte sizes", () => {
    expect(formatModelSize(142)).toBe("142 MB");
    expect(formatModelSize(1624)).toBe("1.6 GB");
    expect(formatBytes(500)).toBe("1 KB");
    expect(formatBytes(5 * 1024 ** 2)).toBe("5 MB");
    expect(formatBytes(2.5 * 1024 ** 3)).toBe("2.50 GB");
  });
});

describe("relative times", () => {
  const now = new Date(2026, 8, 17, 12, 0, 0).getTime();
  it("describes how long ago", () => {
    expect(formatRelative(now - 10_000, now)).toBe("just now");
    expect(formatRelative(now - 5 * 60_000, now)).toBe("5m ago");
    expect(formatRelative(now - 3 * 3_600_000, now)).toBe("3h ago");
    expect(formatRelative(now - 2 * 86_400_000, now)).toBe("2d ago");
    // Clock skew into the future doesn't produce negative values.
    expect(formatRelative(now + 60_000, now)).toBe("just now");
  });

  it("finds the start of the local day", () => {
    expect(startOfDay(now)).toBe(new Date(2026, 8, 17).getTime());
  });
});

describe("models", () => {
  it("names speed and accuracy tiers", () => {
    expect(speedTier(9.7)).toBe("Blazing");
    expect(speedTier(7.5)).toBe("Fast");
    expect(accuracyTier(9.4)).toBe("Flawless");
    expect(accuracyTier(6)).toBe("Rough");
  });

  it("describes language coverage", () => {
    expect(formatLanguages({ englishOnly: true, languages: ["en"] })).toBe("English only");
    expect(formatLanguages({ englishOnly: false, languages: ["de", "fr"] })).toBe("2 languages");
    expect(formatLanguages({ englishOnly: false, languages: null })).toBe("99 languages");
  });
});

describe("text", () => {
  it("counts words", () => {
    expect(countWords("")).toBe(0);
    expect(countWords("  hello   world \n again ")).toBe(3);
  });

  it("splits single-modifier and combined hotkeys", () => {
    expect(hotkeyParts("Fn")).toEqual(["fn"]);
    expect(hotkeyParts("RightCommand")).toEqual(["Right ⌘"]);
    expect(hotkeyParts("Ctrl+Space")).toHaveLength(2);
  });
});
