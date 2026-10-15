import { useId } from "react";
import { cn } from "@/lib/cn";

// Five streaks of rain; the drops they land as trace a "W". Matches the app icon.
const DROPS = [
  { x: 8.6, y: 18.2 },
  { x: 12.3, y: 22.6 },
  { x: 16, y: 19.5 },
  { x: 19.7, y: 22.6 },
  { x: 23.4, y: 18.2 },
];

/** The Wordflow mark: rain falling into a "W" on a deep blue rounded square, like the app icon. */
export function LogoMark({ className }: { className?: string }) {
  const id = useId();
  return (
    <svg viewBox="0 0 32 32" aria-hidden className={cn("shrink-0", className)}>
      <defs>
        <linearGradient id={`${id}-sea`} x1="0.15" y1="0" x2="0.85" y2="1">
          <stop offset="0" stopColor="#3a86ee" />
          <stop offset="0.5" stopColor="#1a4fb0" />
          <stop offset="1" stopColor="#0a1d47" />
        </linearGradient>
        <linearGradient id={`${id}-streak`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#e6f6ff" stopOpacity="0.1" />
          <stop offset="1" stopColor="#ffffff" />
        </linearGradient>
      </defs>
      <rect width="32" height="32" rx="8" fill={`url(#${id}-sea)`} />
      {DROPS.map(({ x, y }) => (
        <g key={x}>
          <rect x={x - 1} y={y - 10} width="2" height="10" rx="1" fill={`url(#${id}-streak)`} />
          <circle cx={x} cy={y} r="1.5" fill="#ffffff" />
        </g>
      ))}
    </svg>
  );
}
