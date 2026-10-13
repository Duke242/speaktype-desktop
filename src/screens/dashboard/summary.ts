import type { DictionaryEntry, OS, RecordingMode, StatsEntry } from "@/lib/api";
import { TYPING_WORDS_PER_MINUTE, formatDay, formatTime, startOfDay } from "@/lib/format";

/** A single-spaced typed page holds about 500 words. */
export const WORDS_PER_PAGE = 500;

export interface Summary {
  today: number;
  totalWords: number;
  /** Whole pages of typing, rounded; 0 until the first full page. */
  pages: number;
  minutesSaved: number;
}

/** The numbers in the dashboard's status line. */
export function summarize(stats: readonly StatsEntry[], now: number): Summary {
  const today = startOfDay(now);
  const totalWords = stats.reduce((sum, s) => sum + s.wordCount, 0);
  return {
    today: stats.filter((s) => s.createdAt >= today).length,
    totalWords,
    pages: totalWords < WORDS_PER_PAGE ? 0 : Math.round(totalWords / WORDS_PER_PAGE),
    minutesSaved: Math.floor(totalWords / TYPING_WORDS_PER_MINUTE),
  };
}

/** "Hold" or "Press", for "Hold fn and talk." Hybrid mode holds too. */
export function hotkeyVerb(mode: RecordingMode) {
  return mode === "toggle" ? "Press" : "Hold";
}

/** What to call the user's machine: "this Mac" on macOS. */
export function thisDevice(os: OS) {
  return os === "macos" ? "this Mac" : "this computer";
}

/** The time column of a feed: the time for today, otherwise the day. */
export function feedTime(ms: number, now: number) {
  return startOfDay(ms) === startOfDay(now) ? formatTime(ms) : formatDay(ms);
}

/** The newest learned words first. Learned entries are appended to the dictionary as they're learned. */
export function recentlyLearned(dictionary: readonly DictionaryEntry[], limit = 3) {
  return dictionary
    .filter((entry) => entry.learned && entry.replacement.trim())
    .slice(-limit)
    .reverse();
}
