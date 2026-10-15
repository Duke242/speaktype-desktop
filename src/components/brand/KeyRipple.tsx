import type { CSSProperties } from "react";
import { cn } from "@/lib/cn";
import { hotkeyParts } from "@/lib/format";

/** Rings around the keys, nearest first: how far each spreads and how strongly it shows. */
const RINGS = [
  { spread: 33, tone: "border-accent/25" },
  { spread: 68, tone: "border-accent/15" },
  { spread: 108, tone: "border-accent/8" },
];

/**
 * The dictation hotkey as large soft keycaps, with rings spreading around
 * them like ripples on water. Rings are circles around a single key and
 * stretch into ovals around a shortcut like Ctrl + Shift + Space.
 * They swell gently unless the user prefers reduced motion.
 */
export function KeyRipple({ hotkey, className }: { hotkey: string; className?: string }) {
  const parts = hotkeyParts(hotkey);
  return (
    <div aria-hidden className={cn("relative inline-flex items-center gap-3", className)}>
      {RINGS.map(({ spread, tone }, i) => (
        <span
          key={spread}
          className={cn("pointer-events-none absolute rounded-full border motion-safe:animate-ripple", tone)}
          style={{ inset: -spread, animationDelay: `${i * -1.8}s` }}
        />
      ))}
      {parts.map((part, i) => (
        <kbd
          key={i}
          className="relative grid h-[84px] min-w-[84px] place-items-center rounded-panel border border-line-strong bg-surface px-6 font-sans type-title text-ink shadow-keycap"
        >
          {part}
        </kbd>
      ))}
    </div>
  );
}

/** The hero's width in SVG units; every wave repeats a whole number of times across it. */
const WAVE_WIDTH = 1124;

/**
 * The hero's waves, faintest last: how many crests fit across, how tall they
 * are, how long one full drift takes, and which way it goes.
 */
const WAVES = [
  { crests: 2, height: 11, y: 60, opacity: 0.3, seconds: 12, reverse: false },
  { crests: 3, height: 9, y: 67, opacity: 0.18, seconds: 16, reverse: true },
  { crests: 4, height: 7, y: 53, opacity: 0.11, seconds: 21, reverse: false },
];

/** A sine-like wave across two hero widths, so sliding it one width left loops without a seam. */
function wavePath(crests: number, height: number, y: number) {
  const half = WAVE_WIDTH / crests / 2;
  let d = `M0 ${y} Q ${half / 2} ${y - height * 2} ${half} ${y}`;
  // Count half-waves rather than adding up floats, so the path always ends exactly at two widths.
  for (let i = 2; i <= crests * 4; i++) d += ` T ${i * half} ${y}`;
  return d;
}

/**
 * Three slow waves behind the keycap that drift sideways at different speeds,
 * crossing as they go. Still if the user prefers reduced motion.
 *
 * Each wave is drawn twice as wide as the hero in its own layer, and the layer
 * slides half its width. Moving an HTML layer rather than the SVG path lets the
 * webview composite the motion instead of repainting every frame.
 */
export function HeroWaves({ className }: { className?: string }) {
  return (
    <div aria-hidden className={cn("pointer-events-none overflow-hidden text-chart fade-out-sides", className)}>
      {WAVES.map((wave) => (
        <div
          key={wave.y}
          className="absolute inset-y-0 left-0 w-[200%] motion-safe:animate-drift"
          style={{
            "--drift-duration": `${wave.seconds}s`,
            animationDirection: wave.reverse ? "reverse" : "normal",
          } as CSSProperties}
        >
          <svg
            viewBox={`0 0 ${WAVE_WIDTH * 2} 120`}
            preserveAspectRatio="none"
            className="size-full"
            fill="none"
            stroke="currentColor"
            strokeWidth={1.5}
          >
            <path
              d={wavePath(wave.crests, wave.height, wave.y)}
              strokeOpacity={wave.opacity}
              vectorEffect="non-scaling-stroke"
            />
          </svg>
        </div>
      ))}
    </div>
  );
}
