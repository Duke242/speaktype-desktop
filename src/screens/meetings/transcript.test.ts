import { describe, expect, it } from "vitest";
import type { Meeting, MeetingSegment, MeetingSource } from "@/lib/api";
import { formatTimestamp, insertSegment, isLabelled, meetingMarkdown, transcriptBlocks } from "./transcript";

const segment = (startSecs: number, source: MeetingSource, text: string): MeetingSegment => ({
  startSecs,
  endSecs: startSecs + 10,
  source,
  text,
});

const meeting = (patch: Partial<Meeting> = {}): Meeting => ({
  id: "m1",
  title: "Weekly sync",
  createdAt: 0,
  durationSecs: 1830,
  status: "done",
  sources: ["microphone", "system"],
  segments: [],
  audioPath: null,
  model: "Whisper Base",
  notes: null,
  notesError: null,
  summarizing: false,
  ...patch,
});

describe("formatTimestamp", () => {
  it("shows minutes, and hours past the first", () => {
    expect(formatTimestamp(0)).toBe("0:00");
    expect(formatTimestamp(5.9)).toBe("0:05");
    expect(formatTimestamp(754)).toBe("12:34");
    expect(formatTimestamp(3725)).toBe("1:02:05");
    expect(formatTimestamp(-3)).toBe("0:00");
  });
});

describe("insertSegment", () => {
  it("keeps time order when the two tracks arrive out of turn", () => {
    let segments: MeetingSegment[] = [];
    for (const s of [segment(30, "system", "c"), segment(0, "microphone", "a"), segment(15, "system", "b")]) {
      segments = insertSegment(segments, s);
    }
    expect(segments.map((s) => s.text)).toEqual(["a", "b", "c"]);
  });

  it("ignores a segment it already has", () => {
    const segments = [segment(0, "microphone", "a")];
    expect(insertSegment(segments, segment(0, "microphone", "a"))).toEqual(segments);
    expect(insertSegment(segments, segment(0, "system", "b"))).toHaveLength(2);
  });
});

describe("transcriptBlocks", () => {
  it("joins one side's turns and starts a block when the speaker changes", () => {
    const blocks = transcriptBlocks(
      [
        segment(0, "microphone", "Shall we start?"),
        segment(10, "microphone", " Okay. "),
        segment(20, "system", "Yes."),
        segment(25, "system", "  "),
        segment(30, "microphone", "Great."),
      ],
      true,
    );
    expect(blocks).toEqual([
      { startSecs: 0, speaker: "You", text: "Shall we start? Okay." },
      { startSecs: 20, speaker: "Others", text: "Yes." },
      { startSecs: 30, speaker: "You", text: "Great." },
    ]);
  });

  it("starts a new block in a long monologue, and has no speakers for one track", () => {
    const segments = [segment(0, "microphone", "a"), segment(60, "microphone", "b"), segment(100, "microphone", "c")];
    expect(transcriptBlocks(segments, false)).toEqual([
      { startSecs: 0, speaker: null, text: "a b" },
      { startSecs: 100, speaker: null, text: "c" },
    ]);
    expect(isLabelled(meeting({ sources: ["microphone"] }))).toBe(false);
    expect(isLabelled(meeting())).toBe(true);
  });
});

describe("meetingMarkdown", () => {
  it("writes notes as a checklist, then the transcript", () => {
    const markdown = meetingMarkdown(
      meeting({
        segments: [segment(5, "microphone", "Let's ship Friday."), segment(12, "system", "Agreed.")],
        notes: { summary: "Shipping Friday.", actionItems: ["Alice writes the notes"], openQuestions: [] },
      }),
      "Monday",
    );
    expect(markdown).toBe(
      [
        "# Weekly sync",
        "",
        "Monday · 30m 30s",
        "",
        "## Summary",
        "",
        "Shipping Friday.",
        "",
        "## Action items",
        "",
        "- [ ] Alice writes the notes",
        "",
        "## Open questions",
        "",
        "None.",
        "",
        "## Transcript",
        "",
        "**[0:05] You:** Let's ship Friday.",
        "",
        "**[0:12] Others:** Agreed.",
        "",
      ].join("\n"),
    );
  });

  it("leaves notes out until they're written", () => {
    const markdown = meetingMarkdown(meeting({ sources: ["microphone"] }), "Monday");
    expect(markdown).not.toContain("## Summary");
    expect(markdown).toContain("Nothing was transcribed.");
  });
});
