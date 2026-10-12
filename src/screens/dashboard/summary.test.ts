import { describe, expect, it } from "vitest";
import type { DictionaryEntry } from "@/lib/api";
import { feedTime, hotkeyVerb, recentlyLearned, summarize, thisDevice } from "./summary";

// formatDay reads the real clock, so dates are relative to today.
const now = Date.now();
const today = new Date(now);
const at = (daysAgo: number, hour: number) =>
  new Date(today.getFullYear(), today.getMonth(), today.getDate() - daysAgo, hour).getTime();

describe("summarize", () => {
  it("counts today, words, pages and minutes saved", () => {
    const stats = [
      { createdAt: at(0, 9), wordCount: 300, durationSecs: 10 },
      { createdAt: at(0, 0), wordCount: 100, durationSecs: 5 },
      { createdAt: at(1, 23), wordCount: 1000, durationSecs: 40 },
    ];
    expect(summarize(stats, now)).toEqual({ today: 2, totalWords: 1400, pages: 3, minutesSaved: 35 });
  });

  it("has no pages before the first full one", () => {
    expect(summarize([], now)).toEqual({ today: 0, totalWords: 0, pages: 0, minutesSaved: 0 });
    expect(summarize([{ createdAt: at(10, 9), wordCount: 499, durationSecs: 1 }], now).pages).toBe(0);
  });
});

describe("wording", () => {
  it("follows the recording mode and the OS", () => {
    expect(hotkeyVerb("hold")).toBe("Hold");
    expect(hotkeyVerb("toggle")).toBe("Press");
    expect(thisDevice("macos")).toBe("this Mac");
    expect(thisDevice("windows")).toBe("this computer");
    expect(thisDevice("linux")).toBe("this computer");
  });

  it("shows a time for today and a day before that", () => {
    expect(feedTime(at(0, 9), now)).not.toMatch(/Today|Yesterday/);
    expect(feedTime(at(1, 9), now)).toBe("Yesterday");
  });
});

describe("recentlyLearned", () => {
  const entry = (id: string, learned?: boolean): DictionaryEntry => ({
    id,
    trigger: id,
    replacement: id.toUpperCase(),
    isEnabled: true,
    matchWholeWord: true,
    learned,
  });

  it("returns the newest learned words first", () => {
    const dictionary = [entry("a", true), entry("b"), entry("c", true), entry("d", true), entry("e", true)];
    expect(recentlyLearned(dictionary).map((e) => e.id)).toEqual(["e", "d", "c"]);
    expect(recentlyLearned([entry("b")])).toEqual([]);
  });
});
