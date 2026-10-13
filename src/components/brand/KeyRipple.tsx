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

/** Three slow waves drawn across the hero, behind the keycap. */
export function HeroWaves({ className }: { className?: string }) {
  return (
    <svg
      aria-hidden
      viewBox="0 0 1124 120"
      preserveAspectRatio="none"
      className={cn("pointer-events-none text-chart", className)}
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
    >
      <path
        d="M0 60 C 90 40, 180 80, 270 60 S 450 40, 540 60 S 720 82, 810 60 S 990 38, 1124 60"
        strokeOpacity={0.28}
        vectorEffect="non-scaling-stroke"
      />
      <path
        d="M0 68 C 110 52, 200 86, 300 68 S 480 50, 580 68 S 760 88, 860 68 S 1020 50, 1124 66"
        strokeOpacity={0.16}
        vectorEffect="non-scaling-stroke"
      />
      <path
        d="M0 52 C 120 36, 220 70, 330 52 S 520 34, 620 52 S 800 72, 900 52 S 1060 36, 1124 50"
        strokeOpacity={0.1}
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}
