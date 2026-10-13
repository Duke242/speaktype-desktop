import { ChevronRight, Mic, Search, Sparkles, Square, TriangleAlert, Users } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { Route } from "@/app/routes";
import {
  Badge,
  Button,
  Callout,
  Card,
  EmptyState,
  GroupHeader,
  Page,
  PageHeader,
  Select,
  Spinner,
  Switch,
  TextField,
  useToast,
} from "@/components/ui";
import { api, errorMessage, type MeetingListItem, type MeetingState } from "@/lib/api";
import { cn } from "@/lib/cn";
import { formatClock, formatDay, formatDuration, formatTime, plural, startOfDay } from "@/lib/format";
import { useNow } from "@/lib/hooks";
import { useStore } from "@/lib/store";
import { MeetingDetail } from "./MeetingDetail";
import { formatTimestamp, isLabelled, transcriptBlocks } from "./transcript";
import { useMeeting, useMeetingList, useMeetingSetup, useMeetingState } from "./useMeetings";

export function MeetingsScreen({ onNavigate }: { onNavigate: (route: Route) => void }) {
  const [openId, setOpenId] = useState<string | null>(null);
  if (openId) return <MeetingDetail id={openId} onBack={() => setOpenId(null)} onNavigate={onNavigate} />;
  return <MeetingList onOpen={setOpenId} />;
}

function MeetingList({ onOpen }: { onOpen: (id: string) => void }) {
  const state = useMeetingState();
  const [query, setQuery] = useState("");
  const items = useMeetingList(query);
  const groups = useMemo(() => groupByDay(items ?? []), [items]);
  const searching = query.trim() !== "";

  return (
    <Page>
      <PageHeader
        title="Meetings"
        description="A live transcript of your calls, with notes afterwards. Everything stays on this computer."
      />

      <Recorder state={state} onOpen={onOpen} />

      {items && items.length === 0 && !searching && (
        <EmptyState
          icon={Users}
          title="No meetings yet"
          description="Start one when your call begins. The transcript appears as people talk."
        />
      )}

      {items && (items.length > 0 || searching) && (
        <>
          <TextField
            icon={Search}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search titles, transcripts and notes"
            aria-label="Search meetings"
            className="mb-10"
          />
          {items.length === 0 && (
            <p className="py-10 text-center type-body text-ink-muted">No meetings match “{query.trim()}”.</p>
          )}
          <div className="flex flex-col gap-10">
            {groups.map(([day, dayItems]) => (
              <section key={day}>
                <GroupHeader
                  title={formatDay(day)}
                  action={<span className="type-caption text-ink-muted">{plural(dayItems.length, "meeting")}</span>}
                />
                <ol>
                  {dayItems.map((item) => (
                    <MeetingRow key={item.id} item={item} onOpen={() => onOpen(item.id)} />
                  ))}
                </ol>
              </section>
            ))}
          </div>
        </>
      )}
    </Page>
  );
}

function groupByDay(items: readonly MeetingListItem[]): [number, MeetingListItem[]][] {
  const byDay = new Map<number, MeetingListItem[]>();
  for (const item of items) {
    const day = startOfDay(item.createdAt);
    byDay.set(day, [...(byDay.get(day) ?? []), item]);
  }
  return [...byDay.entries()];
}

function MeetingRow({ item, onOpen }: { item: MeetingListItem; onOpen: () => void }) {
  return (
    <li className="-mx-4 rounded-card px-4 transition-colors duration-150 not-last:after:block not-last:after:h-px not-last:after:bg-line hover:bg-hover/50">
      <button
        type="button"
        onClick={onOpen}
        className="grid w-full grid-cols-[72px_minmax(0,1fr)_auto] items-baseline gap-5 py-4 text-left"
      >
        <time className="type-small text-ink-muted tabular-nums">{formatTime(item.createdAt)}</time>
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <span className="truncate type-label text-ink">{item.title}</span>
            {item.status === "recording" && <Badge tone="danger">Recording</Badge>}
            {item.status === "processing" && <Badge>Saving…</Badge>}
            {item.hasNotes && (
              <Badge tone="accent" icon={Sparkles}>
                Notes
              </Badge>
            )}
          </div>
          <p className="mt-1 line-clamp-2 type-small text-ink-secondary">
            {item.preview || "Nothing was transcribed."}
          </p>
        </div>
        <span className="flex items-center gap-3 type-caption text-ink-muted tabular-nums">
          {formatDuration(item.durationSecs)}
          <ChevronRight size={16} className="self-center" />
        </span>
      </button>
    </li>
  );
}

/** Start a meeting, or follow the one being recorded. */
function Recorder({ state, onOpen }: { state: MeetingState; onOpen: (id: string) => void }) {
  if (state.phase === "idle") return <StartMeeting />;
  return <LiveMeeting state={state} onOpen={onOpen} />;
}

function StartMeeting() {
  const { settings, updateSettings, models } = useStore();
  const setup = useMeetingSetup();
  const toast = useToast();
  const [title, setTitle] = useState("");
  const [starting, setStarting] = useState(false);
  const meetings = settings.meetings;
  const hasModel = models.some((m) => m.downloaded && m.id === settings.selectedModel);

  const start = () => {
    setStarting(true);
    api
      .startMeeting(title.trim() || `Meeting at ${formatTime(Date.now())}`)
      .then(() => setTitle(""))
      .catch((e) => toast(errorMessage(e), "error"))
      .finally(() => setStarting(false));
  };
  const save = (patch: Partial<typeof meetings>) =>
    updateSettings({ meetings: { ...meetings, ...patch } }).catch((e) => toast(errorMessage(e), "error"));

  const sources = setup?.systemAudioSources ?? [];
  const sourceOptions = sources.map((s) => ({
    value: s.id,
    label: s.name,
    detail: s.id === "" ? undefined : s.kind === "output" ? "Output" : "Input",
  }));
  // A source that's gone, e.g. an unplugged headset, still shows what was chosen.
  if (meetings.systemAudioSource && !sources.some((s) => s.id === meetings.systemAudioSource)) {
    sourceOptions.push({ value: meetings.systemAudioSource, label: "Unavailable device", detail: undefined });
  }

  return (
    <Card padding="lg" className="mb-12">
      <div className="flex items-center gap-3">
        <TextField
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && hasModel && start()}
          placeholder="Meeting title"
          aria-label="Meeting title"
          className="flex-1"
        />
        <Button variant="accent" icon={Mic} loading={starting} disabled={!hasModel} onClick={start}>
          Start meeting
        </Button>
      </div>
      {!hasModel && (
        <p className="mt-3 type-small text-ink-secondary">Download a speech model in AI Models to record meetings.</p>
      )}

      <div className="mt-5 divide-y divide-line border-t border-line">
        <div className="flex items-center gap-6 py-3.5">
          <div className="min-w-0 flex-1">
            <div className="type-label text-ink">Include system audio</div>
            <div className="mt-0.5 type-small text-ink-secondary">
              Records what your computer plays, so the other side is in the transcript as “Others”.
            </div>
          </div>
          <Switch
            label="Include system audio"
            checked={meetings.captureSystemAudio}
            onChange={(captureSystemAudio) => save({ captureSystemAudio })}
          />
        </div>
        {meetings.captureSystemAudio && sourceOptions.length > 0 && (
          <div className="flex items-center gap-6 py-3.5">
            <div className="min-w-0 flex-1">
              <div className="type-label text-ink">Record from</div>
              <div className="mt-0.5 type-small text-ink-secondary">
                {setup?.systemAudioNote ?? "The output your call plays through."}
              </div>
            </div>
            <div className="w-[240px] shrink-0">
              <Select
                aria-label="System audio source"
                value={meetings.systemAudioSource}
                options={sourceOptions}
                onChange={(systemAudioSource) => save({ systemAudioSource })}
              />
            </div>
          </div>
        )}
        <div className="flex items-center gap-6 py-3.5">
          <div className="min-w-0 flex-1">
            <div className="type-label text-ink">Write notes when it ends</div>
            <div className="mt-0.5 type-small text-ink-secondary">
              {setup && !setup.notesAvailable
                ? "Needs AI cleanup, set up in Settings."
                : "A summary, action items and open questions, by your AI cleanup model."}
            </div>
          </div>
          <Switch
            label="Write notes when it ends"
            checked={meetings.summarizeAutomatically}
            onChange={(summarizeAutomatically) => save({ summarizeAutomatically })}
          />
        </div>
      </div>
    </Card>
  );
}

function LiveMeeting({
  state,
  onOpen,
}: {
  state: Exclude<MeetingState, { phase: "idle" }>;
  onOpen: (id: string) => void;
}) {
  const toast = useToast();
  const recording = state.phase === "recording";
  const now = useNow(1000, recording);
  const { meeting } = useMeeting(state.id);
  const [stopping, setStopping] = useState(false);
  const blocks = useMemo(
    () => (meeting ? transcriptBlocks(meeting.segments, isLabelled(meeting)) : []),
    [meeting],
  );

  // Follows the newest words, unless the user scrolled up to read.
  const scroller = useRef<HTMLDivElement>(null);
  const pinned = useRef(true);
  useEffect(() => {
    const el = scroller.current;
    if (el && pinned.current) el.scrollTop = el.scrollHeight;
  }, [blocks]);

  const stop = () => {
    setStopping(true);
    api
      .stopMeeting()
      .catch((e) => toast(errorMessage(e), "error"))
      .finally(() => setStopping(false));
  };
  const title = recording ? state.title : (meeting?.title ?? "Meeting");

  return (
    <Card padding="none" className="mb-12 overflow-hidden">
      <div className="flex items-center gap-4 px-6 pt-5 pb-4">
        <span className="relative flex size-3 shrink-0">
          {recording && <span className="absolute inset-0 animate-ping rounded-full bg-recording opacity-60" />}
          <span className={cn("relative size-3 rounded-full", recording ? "bg-recording" : "bg-line-strong")} />
        </span>
        <div className="min-w-0 flex-1">
          <div className="truncate type-section text-ink">{title}</div>
          <div className="type-small text-ink-secondary tabular-nums">
            {recording
              ? `${formatClock((now - state.startedAtMs) / 1000)} · ${state.systemAudio ? "You and others" : "Microphone only"}`
              : "Transcribing the last few seconds and saving the recording…"}
          </div>
        </div>
        {recording ? (
          <Button variant="primary" icon={Square} loading={stopping} onClick={stop}>
            Stop
          </Button>
        ) : (
          <Spinner size={16} />
        )}
      </div>

      {recording && state.systemError && (
        <div className="px-6 pb-4">
          <Callout icon={TriangleAlert} title="Recording your microphone only">
            {state.systemError}
          </Callout>
        </div>
      )}

      <div
        ref={scroller}
        onScroll={(e) => {
          const el = e.currentTarget;
          pinned.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
        }}
        className="max-h-[360px] min-h-[120px] overflow-y-auto border-t border-line bg-surface-sunken/60 px-6 py-4"
      >
        {blocks.length === 0 ? (
          <p className="py-6 text-center type-small text-ink-muted">
            {recording
              ? "The transcript appears here a few seconds after people start talking."
              : "Nothing was transcribed."}
          </p>
        ) : (
          <ol data-selectable className="flex flex-col gap-3">
            {blocks.map((block) => (
              <li key={`${block.startSecs}-${block.speaker}`} className="grid grid-cols-[56px_minmax(0,1fr)] gap-3">
                <span className="type-caption text-ink-muted tabular-nums">{formatTimestamp(block.startSecs)}</span>
                <p className="type-body text-ink">
                  {block.speaker && <span className="mr-1.5 font-medium text-accent-ink">{block.speaker}</span>}
                  {block.text}
                </p>
              </li>
            ))}
          </ol>
        )}
      </div>

      {!recording && (
        <div className="flex justify-end border-t border-line px-6 py-3">
          <Button size="sm" variant="ghost" onClick={() => onOpen(state.id)}>
            Open meeting
          </Button>
        </div>
      )}
    </Card>
  );
}
