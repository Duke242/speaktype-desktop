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
  const period = WAVE_WIDTH / crests;
  let d = `M0 ${y} Q ${period / 4} ${y - height * 2} ${period / 2} ${y}`;
  for (let x = period; x <= WAVE_WIDTH * 2; x += period / 2) d += ` T ${x} ${y}`;
  return d;
}

/**
 * Three slow waves behind the keycap that drift sideways at different speeds,
 * crossing as they go. Still if the user prefers reduced motion.
 */
export function HeroWaves({ className }: { className?: string }) {
  return (
    <svg
      aria-hidden
      viewBox={`0 0 ${WAVE_WIDTH} 120`}
      preserveAspectRatio="none"
      className={cn("pointer-events-none overflow-hidden text-chart fade-out-sides", className)}
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
    >
      {WAVES.map((wave) => (
        <path
          key={wave.y}
          d={wavePath(wave.crests, wave.height, wave.y)}
          strokeOpacity={wave.opacity}
          vectorEffect="non-scaling-stroke"
          className="motion-safe:animate-drift"
          style={{
            animationDuration: `${wave.seconds}s`,
            animationDirection: wave.reverse ? "reverse" : "normal",
          }}
        />
      ))}
    </svg>
  );
}
