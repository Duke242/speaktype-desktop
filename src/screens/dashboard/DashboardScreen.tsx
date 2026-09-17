import {
  ArrowRight,
  AudioLines,
  Check,
  Clock,
  Copy,
  Cpu,
  Sparkles,
  TriangleAlert,
  type LucideIcon,
} from "lucide-react";
import { useMemo } from "react";
import type { Route } from "@/app/routes";
import { Button, Callout, Card, EmptyState, Hotkey, IconButton, Page, Spinner } from "@/components/ui";
import type { HistoryItem, StatsEntry } from "@/lib/api";
import { cn } from "@/lib/cn";
import {
  TYPING_WORDS_PER_MINUTE,
  formatDuration,
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

function OverviewCard({ stats, now }: { stats: StatsEntry[]; now: number }) {
  const today = startOfDay(now);
  const totalWords = stats.reduce((sum, s) => sum + s.wordCount, 0);
  const todayCount = stats.filter((s) => s.createdAt >= today).length;
  const minutesSaved = Math.floor(totalWords / TYPING_WORDS_PER_MINUTE);
  const average = stats.length ? Math.round(totalWords / stats.length) : 0;

  return (
    <Card padding="lg" className="flex flex-col">
      <h2 className="type-section">{greeting(new Date(now).getHours())}</h2>
      <div className="mt-4 flex items-baseline gap-3">
        <span className="type-display tabular-nums">{formatNumber(totalWords)}</span>
        <span className="type-body-lg text-ink-secondary">words transcribed</span>
      </div>
      {minutesSaved > 0 ? (
        <div className="mt-3 inline-flex items-center gap-1.5 self-start rounded-full bg-accent-soft px-2.5 py-1 type-small font-medium text-accent-ink">
          <Clock size={13} strokeWidth={2.25} />
          {formatDuration(minutesSaved * 60)} of typing saved
        </div>
      ) : (
        <p className="mt-2 type-small text-ink-muted">Your words and time saved will add up here.</p>
      )}

      <dl className="mt-auto grid grid-cols-3 divide-x divide-line-subtle border-t border-line-subtle pt-6">
        <Stat label="Today" value={formatNumber(todayCount)} className="pl-0" />
        <Stat label="All time" value={formatNumber(stats.length)} />
        <Stat label="Words per note" value={formatNumber(average)} />
      </dl>
    </Card>
  );
}

function Stat({ label, value, className }: { label: string; value: string; className?: string }) {
  return (
    <div className={cn("px-6", className)}>
      <dt className="type-caption text-ink-muted">{label}</dt>
      <dd className="mt-1 type-metric">{value}</dd>
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
          {items.slice(0, 5).map((item, i, shown) => (
            <li
              key={item.id}
              className="group relative flex gap-4 rounded-control px-3 py-3 transition-colors hover:bg-hover/60"
            >
              <div className="w-[76px] shrink-0 pt-px text-right">
                <div className="type-small font-medium text-ink tabular-nums">{formatRelative(item.createdAt, now)}</div>
                <div className="mt-0.5 type-caption text-ink-muted tabular-nums">
                  {item.wordCount} {item.wordCount === 1 ? "word" : "words"}
                </div>
              </div>

              {/* Timeline rail: a dot per transcript, joined by a hairline. */}
              <div className="relative w-2 shrink-0">
                {i > 0 && <span className="absolute -top-3 left-1/2 h-[21px] w-px -translate-x-1/2 bg-line" />}
                <span
                  className={cn(
                    "absolute top-[9px] left-1/2 size-2 -translate-x-1/2 rounded-full ring-4 ring-surface",
                    i === 0 ? "bg-accent" : "bg-line-strong",
                  )}
                />
                {i < shown.length - 1 && (
                  <span className="absolute top-[17px] -bottom-3 left-1/2 w-px -translate-x-1/2 bg-line" />
                )}
              </div>

              <p data-selectable className="line-clamp-2 min-w-0 flex-1 pt-px type-body text-ink">
                {item.transcript}
              </p>

              <IconButton
                icon={copiedKey === item.id ? Check : Copy}
                label={copiedKey === item.id ? "Copied" : "Copy"}
                size="sm"
                onClick={() => copy(item.transcript, item.id)}
                className={cn(
                  "-my-1 opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100",
                  copiedKey === item.id && "text-success opacity-100",
                )}
              />
            </li>
          ))}
        </ol>
      )}
    </Card>
  );
}
