import { describe, expect, it } from "vitest";
import type { DictionaryEntry } from "./api";
import { learnedMessage } from "./useLearnedToast";

const entry = (replacement: string): DictionaryEntry => ({
  id: replacement,
  trigger: replacement.toLowerCase(),
  replacement,
  isEnabled: true,
  matchWholeWord: true,
  learned: true,
});

describe("learnedMessage", () => {
  it("names the first word and counts the rest", () => {
    expect(learnedMessage([])).toBe("");
    expect(learnedMessage([entry("Karan")])).toBe("Learned “Karan”");
    expect(learnedMessage([entry("Karan"), entry("Kubernetes"), entry("Wordflow")])).toBe(
      "Learned “Karan” and 2 more",
    );
  });
});
