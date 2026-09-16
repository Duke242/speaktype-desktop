import { CalendarDays, ChartColumn, Clock, FileText, Hash, Star, TrendingUp, type LucideIcon } from "lucide-react";
import { useMemo, useState } from "react";
import { BarChart, type Bar } from "@/components/BarChart";
import { Card, EmptyState, IconTile, Page, PageHeader, SegmentedControl, type Tone } from "@/components/ui";
import type { StatsEntry } from "@/lib/api";
import { formatDuration, formatNumber, startOfDay } from "@/lib/format";
import { useDictationState, useHistory, useNow } from "@/lib/hooks";

type Period = "week" | "month" | "year";

const PERIOD_DAYS: Record<Period, number> = { week: 7, month: 30, year: 365 };

interface Bucket extends Bar {
  count: number;
  start: number;
}

/** Daily buckets for week and month, monthly buckets for year, oldest first, zero-filled. */
function bucketize(stats: StatsEntry[], period: Period, now: number): Bucket[] {
  const buckets: Bucket[] = [];
  const today = new Date(startOfDay(now));

  if (period === "year") {
    for (let i = 11; i >= 0; i--) {
      const start = new Date(today.getFullYear(), today.getMonth() - i, 1);
      const end = new Date(today.getFullYear(), today.getMonth() - i + 1, 1);
      buckets.push(bucket(stats, start, end, start.toLocaleDateString([], { month: "short" }),
        start.toLocaleDateString([], { month: "long", year: "numeric" })));
    }
    return buckets;
  }

  for (let i = PERIOD_DAYS[period] - 1; i >= 0; i--) {
    const start = new Date(today.getFullYear(), today.getMonth(), today.getDate() - i);
    const end = new Date(today.getFullYear(), today.getMonth(), today.getDate() - i + 1);
    const label =
      period === "week"
        ? start.toLocaleDateString([], { weekday: "short" })
        : start.toLocaleDateString([], { month: "short", day: "numeric" });
    buckets.push(bucket(stats, start, end, label, start.toLocaleDateString([], { weekday: "long", month: "long", day: "numeric" })));
  }
  return buckets;
}

function bucket(stats: StatsEntry[], start: Date, end: Date, label: string, title: string): Bucket {
  const inRange = stats.filter((s) => s.createdAt >= start.getTime() && s.createdAt < end.getTime());
  const count = inRange.length;
  return {
    key: String(start.getTime()),
    start: start.getTime(),
    label,
    title,
    value: inRange.reduce((sum, s) => sum + s.wordCount, 0),
    count,
    detail: `${count} ${count === 1 ? "transcription" : "transcriptions"}`,
  };
}

export function StatisticsScreen() {
  const { stats } = useHistory();
  const dictation = useDictationState();
  const recording = dictation.phase === "recording";
  // Tick every second while recording so total time grows live.
  const now = useNow(recording ? 1000 : 60_000);
  const [period, setPeriod] = useState<Period>("week");

  const buckets = useMemo(() => bucketize(stats, period, now), [stats, period, now]);
  const rangeStart = buckets[0]?.start ?? now;
  const inRange = stats.filter((s) => s.createdAt >= rangeStart);

  const totalWords = buckets.reduce((sum, b) => sum + b.value, 0);
  const count = inRange.length;
  const best = buckets.reduce((a, b) => (b.value > a.value ? b : a), buckets[0]);
  const perBucket = Math.round(totalWords / Math.max(1, buckets.length));
  let seconds = inRange.reduce((sum, s) => sum + s.durationSecs, 0);
  if (dictation.phase === "recording" && dictation.startedAtMs >= rangeStart) {
    seconds += (now - dictation.startedAtMs) / 1000;
  }

  const periodWord = period === "year" ? "month" : "day";
  const noun = { week: "this week", month: "in the last 30 days", year: "in the last 12 months" }[period];

  return (
    <Page>
      <PageHeader
        title="Statistics"
        description={`${formatNumber(totalWords)} words ${noun}`}
        actions={
          <SegmentedControl
            value={period}
            onChange={setPeriod}
            options={[
              { value: "week", label: "Week" },
              { value: "month", label: "Month" },
              { value: "year", label: "Year" },
            ]}
            className="w-[240px]"
          />
        }
      />

      <div className="grid grid-cols-4 gap-4">
        <StatCard icon={FileText} tone="brand" label="Total words" value={formatNumber(totalWords)} />
        <StatCard icon={CalendarDays} tone="neutral" label={`Average per ${periodWord}`} value={formatNumber(perBucket)} />
        <StatCard icon={TrendingUp} tone="neutral" label={`Best ${periodWord}`} value={formatNumber(best?.value ?? 0)} />
        <StatCard icon={Hash} tone="neutral" label="Transcriptions" value={formatNumber(count)} />
      </div>

      <Card padding="lg" className="mt-5">
        <div className="mb-6 flex items-start">
          <div className="flex-1">
            <h2 className="type-section">Words dictated</h2>
            <p className="mt-0.5 type-small text-ink-secondary">Per {periodWord}, {noun}</p>
          </div>
          <div className="text-right">
            <div className="type-label tabular-nums">
              {formatNumber(count)} {count === 1 ? "transcription" : "transcriptions"}
            </div>
            <div className="type-caption text-ink-muted tabular-nums">{formatDuration(seconds)} recorded</div>
          </div>
        </div>
        {totalWords === 0 ? (
          <EmptyState
            icon={ChartColumn}
            title="No activity yet"
            description="Your dictation stats will appear here."
            className="h-[288px] justify-center py-0"
          />
        ) : (
          <BarChart bars={buckets} unit="words" labelEvery={period === "month" ? 7 : 1} />
        )}
      </Card>

      <div className="mt-5 grid grid-cols-3 gap-4">
        <DetailCard icon={Hash} tone="neutral" label="Average words per note" value={formatNumber(count ? Math.round(totalWords / count) : 0)} />
        <DetailCard
          icon={Star}
          tone="neutral"
          label={`Most active ${periodWord}`}
          value={best && best.value > 0 ? best.title : "Not yet"}
        />
        <DetailCard icon={Clock} tone="neutral" label="Time recorded" value={formatDuration(seconds)} />
      </div>
    </Page>
  );
}

function StatCard({ icon, tone, label, value }: { icon: LucideIcon; tone: Tone; label: string; value: string }) {
  return (
    <Card padding="md">
      <IconTile icon={icon} tone={tone} />
      <div className="mt-4 type-metric">{value}</div>
      <div className="type-small text-ink-secondary">{label}</div>
    </Card>
  );
}

function DetailCard({ icon, tone, label, value }: { icon: LucideIcon; tone: Tone; label: string; value: string }) {
  return (
    <Card padding="none" className="flex items-center gap-3 px-4 py-3.5">
      <IconTile icon={icon} tone={tone} size="md" />
      <div className="min-w-0">
        <div className="type-caption text-ink-muted">{label}</div>
        <div className="truncate type-label tabular-nums">{value}</div>
      </div>
    </Card>
  );
}
