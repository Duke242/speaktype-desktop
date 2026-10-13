import type { Meeting, MeetingSegment, MeetingSource } from "@/lib/api";
import { formatDuration } from "@/lib/format";

/** 5 → "0:05", 3725 → "1:02:05". The same timestamps the notes model reads. */
export function formatTimestamp(totalSeconds: number) {
  const s = Math.max(0, Math.floor(totalSeconds));
  const mm = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, "0");
  return s >= 3600 ? `${Math.floor(s / 3600)}:${String(mm).padStart(2, "0")}:${ss}` : `${mm}:${ss}`;
}

export function speakerLabel(source: MeetingSource) {
  return source === "microphone" ? "You" : "Others";
}

/** Speakers are only told apart when both the microphone and system audio were recorded. */
export function isLabelled(meeting: Pick<Meeting, "sources">) {
  return meeting.sources.includes("microphone") && meeting.sources.includes("system");
}

/** Adds a segment in time order, ignoring one already there, as the core does. */
export function insertSegment(segments: readonly MeetingSegment[], segment: MeetingSegment): MeetingSegment[] {
  if (segments.some((s) => s.startSecs === segment.startSecs && s.source === segment.source)) return [...segments];
  let at = segments.length;
  while (at > 0 && segments[at - 1].startSecs > segment.startSecs) at--;
  return [...segments.slice(0, at), segment, ...segments.slice(at)];
}

export interface TranscriptBlock {
  startSecs: number;
  /** "You" or "Others", or null when speakers aren't told apart. */
  speaker: string | null;
  text: string;
}

/** A block covers at most this much of the meeting, so timestamps stay useful in a long monologue. */
const BLOCK_SECS = 90;

/** Joins consecutive segments from the same side into readable blocks. */
export function transcriptBlocks(segments: readonly MeetingSegment[], labelled: boolean): TranscriptBlock[] {
  const blocks: TranscriptBlock[] = [];
  let last: { block: TranscriptBlock; source: MeetingSource } | null = null;
  for (const segment of segments) {
    const text = segment.text.trim();
    if (!text) continue;
    const sameSide = last && (!labelled || last.source === segment.source);
    if (last && sameSide && segment.startSecs - last.block.startSecs < BLOCK_SECS) {
      last.block.text += ` ${text}`;
      continue;
    }
    const block = { startSecs: segment.startSecs, speaker: labelled ? speakerLabel(segment.source) : null, text };
    blocks.push(block);
    last = { block, source: segment.source };
  }
  return blocks;
}

/** "Tuesday, 13 October 2026, 10:30" in the user's locale. */
export function formatMeetingDate(ms: number) {
  return new Date(ms).toLocaleString([], { dateStyle: "full", timeStyle: "short" });
}

/** Notes and transcript as Markdown, for copying or saving. */
export function meetingMarkdown(meeting: Meeting, date = formatMeetingDate(meeting.createdAt)) {
  const lines = [`# ${meeting.title}`, "", `${date} · ${formatDuration(meeting.durationSecs)}`, ""];
  const notes = meeting.notes;
  if (notes) {
    lines.push("## Summary", "", notes.summary || "No summary.", "");
    lines.push("## Action items", "");
    lines.push(...(notes.actionItems.length ? notes.actionItems.map((item) => `- [ ] ${item}`) : ["None."]));
    lines.push("", "## Open questions", "");
    lines.push(...(notes.openQuestions.length ? notes.openQuestions.map((q) => `- ${q}`) : ["None."]));
    lines.push("");
  }
  lines.push("## Transcript", "");
  const blocks = transcriptBlocks(meeting.segments, isLabelled(meeting));
  if (blocks.length === 0) lines.push("Nothing was transcribed.", "");
  for (const block of blocks) {
    const speaker = block.speaker ? ` ${block.speaker}:` : "";
    lines.push(`**[${formatTimestamp(block.startSecs)}]${speaker}** ${block.text}`, "");
  }
  return `${lines.join("\n").trimEnd()}\n`;
}
