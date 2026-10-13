import { ChartColumn } from "lucide-react";
import { useMemo, useState } from "react";
import { BarChart, type Bar } from "@/components/BarChart";
import { EmptyState, GroupHeader, Page, PageHeader, SegmentedControl } from "@/components/ui";
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
    <Page width="wide">
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

      <dl className="grid grid-cols-4 divide-x divide-line">
        <Figure label="Total words" value={formatNumber(totalWords)} />
        <Figure label={`Average per ${periodWord}`} value={formatNumber(perBucket)} />
        <Figure label={`Best ${periodWord}`} value={formatNumber(best?.value ?? 0)} />
        <Figure label="Transcriptions" value={formatNumber(count)} />
      </dl>

      <section className="mt-14">
        <GroupHeader
          title="Words dictated"
          description={`Per ${periodWord}, ${noun}`}
          action={
            <span className="type-caption text-ink-muted tabular-nums">
              {formatNumber(count)} {count === 1 ? "transcription" : "transcriptions"} · {formatDuration(seconds)}{" "}
              recorded
            </span>
          }
        />
        <div className="pt-8">
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
        </div>
      </section>

      <dl className="mt-14 grid grid-cols-3 divide-x divide-line border-t border-line pt-6">
        <Detail label="Average words per note" value={formatNumber(count ? Math.round(totalWords / count) : 0)} />
        <Detail label={`Most active ${periodWord}`} value={best && best.value > 0 ? best.title : "Not yet"} />
        <Detail label="Time recorded" value={formatDuration(seconds)} />
      </dl>
    </Page>
  );
}

/** A headline number over its label, separated from its neighbours by a hairline. */
function Figure({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col-reverse px-6 first:pl-0">
      <dt className="mt-1 type-small text-ink-secondary">{label}</dt>
      <dd className="type-metric">{value}</dd>
    </div>
  );
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 px-6 first:pl-0">
      <dt className="type-caption text-ink-muted">{label}</dt>
      <dd className="mt-0.5 truncate type-label tabular-nums">{value}</dd>
    </div>
  );
}
