import {
  ArrowRight,
  AudioLines,
  Check,
  Clock,
  Copy,
  Cpu,
  Hash,
  Mic,
  Pause,
  Play,
  Sparkles,
  TriangleAlert,
  type LucideIcon,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Route } from "@/app/routes";
import { Button, Callout, Card, EmptyState, Hotkey, IconButton, IconTile, Page, Spinner, type Tone } from "@/components/ui";
import { api, type HistoryItem, type StatsEntry } from "@/lib/api";
import { cn } from "@/lib/cn";
import {
  TYPING_WORDS_PER_MINUTE,
  formatDuration,
  formatMinutesSaved,
  formatNumber,
  formatRelative,
  startOfDay,
} from "@/lib/format";
import { useCopy, useEngineStatus, useHistory, useNow } from "@/lib/hooks";
import { useStore } from "@/lib/store";

export function DashboardScreen({ onNavigate }: { onNavigate: (route: Route) => void }) {
  const { items, stats } = useHistory();
  const now = useNow(60_000);

  return (
    <Page>
      <ReadinessBar onNavigate={onNavigate} />
      <div className="grid grid-cols-[minmax(0,1fr)_380px] gap-5">
        <OverviewCard stats={stats} now={now} />
        <WeekCard stats={stats} now={now} />
      </div>
      <RecentCard items={items} now={now} onNavigate={onNavigate} />
    </Page>
  );
}

function greeting(hour: number) {
  if (hour >= 6 && hour < 12) return "Good morning";
  if (hour >= 12 && hour < 17) return "Good afternoon";
  if (hour >= 17 && hour < 22) return "Good evening";
  return "Welcome back";
}

/** Shows how to dictate, which model is ready, and anything blocking dictation. */
function ReadinessBar({ onNavigate }: { onNavigate: (route: Route) => void }) {
  const { settings, status, models } = useStore();
  const engine = useEngineStatus();
  const model = models.find((m) => m.id === settings.selectedModel && m.downloaded);

  const notices: { icon: LucideIcon; text: string }[] = [];
  if (status.hotkeyError) notices.push({ icon: TriangleAlert, text: `${status.hotkeyError}. Choose another hotkey in Settings.` });
  for (const note of status.setupNotes) notices.push({ icon: TriangleAlert, text: note });

  return (
    <div className="mb-6 space-y-3">
      {!model ? (
        <Callout
          icon={Sparkles}
          tone="brand"
          title="Download a model to start dictating"
          action={
            <Button variant="accent" size="sm" icon={ArrowRight} onClick={() => onNavigate("models")}>
              Choose a model
            </Button>
          }
        >
          Models run entirely on this computer. Your voice never leaves it.
        </Callout>
      ) : (
        <div className="flex items-center gap-4 rounded-card border border-line bg-surface px-5 py-3.5">
          <div className="flex items-center gap-2.5 type-body text-ink-secondary">
            {settings.recordingMode === "hold" ? "Hold" : "Press"}
            <Hotkey value={settings.hotkey} />
            to dictate anywhere
          </div>
          <div className="flex-1" />
          <div className="flex items-center gap-2 type-small text-ink-secondary">
            {engine.loading === model.id ? (
              <>
                <Spinner size={12} className="text-accent-ink" />
                Loading {model.name}
              </>
            ) : (
              <>
                <span className="relative flex size-2">
                  <span
                    className={cn(
                      "absolute inline-flex size-full rounded-full",
                      engine.loaded === model.id ? "bg-success" : "bg-ink-disabled",
                    )}
                  />
                </span>
                <Cpu size={14} className="text-ink-muted" />
                {model.name}
              </>
            )}
          </div>
        </div>
      )}
      {notices.map((notice) => (
        <Callout key={notice.text} icon={notice.icon}>
          {notice.text}
        </Callout>
      ))}
    </div>
  );
}

/** A single-spaced typed page holds about 500 words. */
const WORDS_PER_PAGE = 500;

function pagesLine(words: number) {
  if (words === 0) return "Your words and time saved will add up here.";
  if (words < WORDS_PER_PAGE) return `${formatNumber(WORDS_PER_PAGE - words)} words to go until your first full page.`;
  const pages = Math.round(words / WORDS_PER_PAGE);
  return `That's about ${formatNumber(pages)} ${pages === 1 ? "page" : "pages"} you didn't have to type.`;
}

function OverviewCard({ stats, now }: { stats: StatsEntry[]; now: number }) {
  const today = startOfDay(now);
  const totalWords = stats.reduce((sum, s) => sum + s.wordCount, 0);
  const todayCount = stats.filter((s) => s.createdAt >= today).length;
  const minutesSaved = Math.floor(totalWords / TYPING_WORDS_PER_MINUTE);
  const average = stats.length ? Math.round(totalWords / stats.length) : 0;

  return (
    <Card padding="lg" className="relative flex flex-col overflow-hidden">
      <h2 className="relative type-section">{greeting(new Date(now).getHours())}</h2>
      <div className="relative mt-4 flex items-baseline gap-3">
        <span className="type-display tabular-nums">
          {formatNumber(totalWords)}
        </span>
        <span className="type-body-lg text-ink-secondary">words transcribed</span>
      </div>
      <p className="relative mt-2 type-small text-ink-muted">
        {pagesLine(totalWords)}
      </p>

      <div className="relative mt-7 grid grid-cols-2 gap-x-6 gap-y-5 border-t border-line-subtle pt-6">
        <Stat icon={Mic} tone="neutral" value={formatNumber(todayCount)} label="Transcriptions today" />
        <Stat icon={AudioLines} tone="neutral" value={formatNumber(stats.length)} label="Total transcriptions" />
        <Stat icon={Clock} tone="neutral" value={formatMinutesSaved(minutesSaved)} label="Time saved typing" />
        <Stat icon={Hash} tone="neutral" value={formatNumber(average)} label="Average words per note" />
      </div>
    </Card>
  );
}

function Stat({ icon, tone, value, label }: { icon: LucideIcon; tone: Tone; value: string; label: string }) {
  return (
    <div className="flex items-center gap-3">
      <IconTile icon={icon} tone={tone} size="md" />
      <div className="min-w-0">
        <div className="type-metric">{value}</div>
        <div className="type-caption text-ink-muted">{label}</div>
      </div>
    </div>
  );
}

function WeekCard({ stats, now }: { stats: StatsEntry[]; now: number }) {
  const days = useMemo(() => {
    const today = startOfDay(now);
    return Array.from({ length: 7 }, (_, i) => {
      const date = new Date(today);
      date.setDate(date.getDate() - (6 - i));
      const start = date.getTime();
      const end = new Date(start).setDate(date.getDate() + 1);
      return {
        start,
        label: date.toLocaleDateString([], { weekday: "short" }),
        count: stats.filter((s) => s.createdAt >= start && s.createdAt < end).length,
      };
    });
  }, [stats, now]);

  const total = days.reduce((sum, d) => sum + d.count, 0);
  const max = Math.max(1, ...days.map((d) => d.count));
  const busiest = days.reduce((a, b) => (b.count > a.count ? b : a));

  return (
    <Card padding="lg" className="flex flex-col">
      <h2 className="type-section">This week</h2>
      <p className="mt-1 type-small text-ink-secondary">
        <span className="font-medium text-ink tabular-nums">{total}</span> transcriptions
        {total > 0 && (
          <span className="text-ink-muted">
            {" "}
            · Most active on {new Date(busiest.start).toLocaleDateString([], { weekday: "long" })}
          </span>
        )}
      </p>

      <div className="mt-auto flex h-[168px] items-end gap-3 pt-6">
        {days.map((day, i) => {
          const isToday = i === days.length - 1;
          return (
            <div key={day.start} className="flex h-full flex-1 flex-col items-center justify-end gap-2">
              <span className="h-4 type-caption text-ink-muted tabular-nums">{day.count || ""}</span>
              <div
                className={cn(
                  "w-full max-w-6 rounded-t-inner transition-[height] duration-500 ease-out-soft",
                  day.count > 0 ? (isToday ? "bg-chart" : "bg-chart/55") : "bg-hover",
                )}
                style={{ height: Math.max(6, (day.count / max) * 112) }}
              />
              <span className={cn("type-caption", isToday ? "font-medium text-ink" : "text-ink-muted")}>
                {day.label}
              </span>
            </div>
          );
        })}
      </div>
    </Card>
  );
}

/** Plays one history recording at a time. */
function usePlayback() {
  const current = useRef<{ id: string; audio: HTMLAudioElement; url: string } | null>(null);
  const request = useRef(0);
  const [playingId, setPlayingId] = useState<string | null>(null);

  const stop = useCallback(() => {
    request.current++;
    if (current.current) {
      current.current.audio.pause();
      URL.revokeObjectURL(current.current.url);
      current.current = null;
    }
    setPlayingId(null);
  }, []);

  const toggle = useCallback(
    async (id: string) => {
      const wasPlaying = current.current?.id === id;
      stop();
      if (wasPlaying) return;
      const token = request.current;
      try {
        const buffer = await api.readHistoryAudio(id);
        if (token !== request.current) return;
        const url = URL.createObjectURL(new Blob([buffer], { type: "audio/wav" }));
        const audio = new Audio(url);
        audio.onended = stop;
        current.current = { id, audio, url };
        setPlayingId(id);
        await audio.play();
      } catch {
        if (token === request.current) stop();
      }
    },
    [stop],
  );

  useEffect(() => stop, [stop]);
  return { playingId, toggle };
}

const RECENT_COUNT = 10;

function RecentCard({
  items,
  now,
  onNavigate,
}: {
  items?: HistoryItem[];
  now: number;
  onNavigate: (route: Route) => void;
}) {
  const { settings } = useStore();
  const { copy, copiedKey } = useCopy();
  const playback = usePlayback();

  return (
    <Card padding="none" className="mt-5">
      <div className="flex items-center px-6 pt-5 pb-3">
        <h2 className="flex-1 type-section">Recent transcriptions</h2>
        {items && items.length > 0 && (
          <Button variant="ghost" size="sm" onClick={() => onNavigate("history")}>
            View all
            <ArrowRight size={14} />
          </Button>
        )}
      </div>

      {items && items.length === 0 && (
        <EmptyState
          icon={AudioLines}
          title="No transcriptions yet"
          description={
            <span className="inline-flex flex-wrap items-center justify-center gap-1.5">
              {settings.recordingMode === "hold" ? "Hold" : "Press"} <Hotkey value={settings.hotkey} /> in any app
              and start talking.
            </span>
          }
          className="pt-6 pb-12"
        />
      )}

      {items && items.length > 0 && (
        <ol className="px-3 pb-3">
          {items.slice(0, RECENT_COUNT).map((item) => {
            const copied = copiedKey === item.id;
            const playing = playback.playingId === item.id;
            return (
              <li
                key={item.id}
                className={cn(
                  "group relative",
                  // Hairline between rows, hidden next to the highlighted row.
                  "not-first:before:absolute not-first:before:inset-x-3 not-first:before:top-0 not-first:before:h-px not-first:before:bg-line-subtle",
                  "hover:before:opacity-0 [&:hover+li]:before:opacity-0",
                )}
              >
                <button
                  type="button"
                  onClick={() => copy(item.transcript, item.id)}
                  className="flex w-full cursor-pointer items-start gap-6 rounded-control py-3.5 pr-16 pl-3 text-left transition-colors hover:bg-hover/60 active:bg-hover"
                >
                  <div className="w-[88px] shrink-0 pt-px">
                    <div className="type-small font-medium text-ink tabular-nums">
                      {formatRelative(item.createdAt, now)}
                    </div>
                    {/* The word count turns into the copy hint on hover. */}
                    <div className="mt-0.5 grid type-caption">
                      <span
                        className={cn(
                          "col-start-1 row-start-1 text-ink-muted tabular-nums transition-opacity group-hover:opacity-0",
                          copied && "opacity-0",
                        )}
                      >
                        {item.wordCount} {item.wordCount === 1 ? "word" : "words"}
                      </span>
                      <span
                        className={cn(
                          "col-start-1 row-start-1 inline-flex items-center gap-1 font-medium whitespace-nowrap transition-opacity",
                          copied ? "text-success" : "text-accent-ink opacity-0 group-hover:opacity-100",
                        )}
                      >
                        {copied ? <Check size={12} strokeWidth={2.5} /> : <Copy size={12} strokeWidth={2.25} />}
                        {copied ? "Copied" : "Click to copy"}
                      </span>
                    </div>
                  </div>
                  <p className="line-clamp-2 min-w-0 flex-1 pt-px type-body text-ink">{item.transcript}</p>
                </button>

                {/* The recording length turns into a play button on hover. */}
                <div className="pointer-events-none absolute inset-y-0 right-3 grid w-10 items-center justify-items-end">
                  <span
                    className={cn(
                      "col-start-1 row-start-1 type-caption text-ink-muted tabular-nums transition-opacity",
                      item.audioPath && "group-hover:opacity-0",
                      playing && "opacity-0",
                    )}
                  >
                    {formatDuration(item.durationSecs)}
                  </span>
                  {item.audioPath && (
                    <IconButton
                      icon={playing ? Pause : Play}
                      label={playing ? "Stop" : "Play recording"}
                      size="sm"
                      onClick={() => playback.toggle(item.id)}
                      className={cn(
                        "pointer-events-auto col-start-1 row-start-1 transition-[opacity,background-color,color]",
                        playing ? "bg-hover text-ink" : "opacity-0 group-hover:opacity-100 focus-visible:opacity-100",
                      )}
                    />
                  )}
                </div>
              </li>
            );
          })}
        </ol>
      )}
    </Card>
  );
}
