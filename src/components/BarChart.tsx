import { useState } from "react";
import { cn } from "@/lib/cn";
import { formatNumber } from "@/lib/format";

export interface Bar {
  key: string;
  /** Short axis label. */
  label: string;
  /** Tooltip heading, e.g. a full date. */
  title: string;
  value: number;
  /** Tooltip line under the value. */
  detail?: string;
}

/** Rounds the axis maximum up to a clean number and returns evenly spaced ticks. */
function niceTicks(max: number, count = 4) {
  if (max <= 0) return [0, 1, 2, 3, 4];
  const raw = max / count;
  const magnitude = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 2.5, 5, 10].map((m) => m * magnitude).find((s) => s >= raw) ?? raw;
  return Array.from({ length: count + 1 }, (_, i) => Math.round(i * step));
}

/**
 * Single-series column chart: one hue, hairline grid, 4px rounded caps,
 * bars capped at 24px wide, and a tooltip per column.
 */
export function BarChart({
  bars,
  unit,
  height = 260,
  labelEvery = 1,
}: {
  bars: Bar[];
  unit: string;
  height?: number;
  labelEvery?: number;
}) {
  const [hovered, setHovered] = useState<number | null>(null);
  const ticks = niceTicks(Math.max(0, ...bars.map((b) => b.value)));
  const top = ticks[ticks.length - 1];
  const active = hovered !== null ? bars[hovered] : null;

  return (
    <div className="flex gap-3">
      {/* Y axis */}
      <div className="relative mt-2 w-10 shrink-0" style={{ height }}>
        {ticks.map((tick) => (
          <span
            key={tick}
            className="absolute right-0 -translate-y-1/2 type-caption text-ink-muted tabular-nums"
            style={{ bottom: `${(tick / top) * 100}%` }}
          >
            {formatNumber(tick)}
          </span>
        ))}
      </div>

      <div className="min-w-0 flex-1">
        <div className="relative mt-2" style={{ height }} onMouseLeave={() => setHovered(null)}>
          {ticks.map((tick) => (
            <div
              key={tick}
              className={cn("absolute inset-x-0 h-px", tick === 0 ? "bg-line" : "bg-chart-grid")}
              style={{ bottom: `${(tick / top) * 100}%` }}
            />
          ))}

          <div className="absolute inset-0 flex items-end">
            {bars.map((bar, i) => (
              <div
                key={bar.key}
                onMouseEnter={() => setHovered(i)}
                className="flex h-full flex-1 items-end justify-center px-px"
              >
                <div
                  className={cn(
                    "w-full max-w-6 rounded-t-inner bg-chart transition-[height,opacity] duration-500 ease-out-soft",
                    hovered !== null && hovered !== i && "opacity-60",
                  )}
                  style={{ height: bar.value > 0 ? `max(2px, ${(bar.value / top) * 100}%)` : 0 }}
                />
              </div>
            ))}
          </div>

          {active && hovered !== null && (
            <div
              className="pointer-events-none absolute z-10 -translate-x-1/2 animate-fade-in rounded-control border border-line bg-surface px-3 py-2 whitespace-nowrap shadow-elevated"
              style={{
                left: `clamp(60px, ${((hovered + 0.5) / bars.length) * 100}%, calc(100% - 60px))`,
                bottom: `calc(${(active.value / top) * 100}% + 10px)`,
              }}
            >
              <div className="type-caption text-ink-muted">{active.title}</div>
              <div className="type-label text-ink tabular-nums">
                {formatNumber(active.value)} {unit}
              </div>
              {active.detail && <div className="type-caption text-ink-secondary">{active.detail}</div>}
            </div>
          )}
        </div>

        {/* X axis */}
        <div className="mt-2 flex">
          {bars.map((bar, i) => (
            <span
              key={bar.key}
              className={cn(
                "flex-1 text-center type-caption whitespace-nowrap text-ink-muted",
                i === hovered && "text-ink",
              )}
            >
              {(bars.length - 1 - i) % labelEvery === 0 ? bar.label : ""}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}
