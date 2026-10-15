import { useId } from "react";
import { cn } from "@/lib/cn";

// One flowing ribbon shaped like a "W". Matches the app icon (icons/wordflow.svg scaled to 32).
const RIBBON =
  "M5.05 8.93 C6.99 8.93 7.77 23.3 10.87 23.3 S14.37 12.43 16 12.43 S18.64 23.3 21.13 23.3 S25 8.93 26.95 8.93";

/** The WordFlow mark: a flowing "W" on a teal-to-blue rounded square, like the app icon. */
export function LogoMark({ className }: { className?: string }) {
  const id = useId();
  return (
    <svg viewBox="0 0 32 32" aria-hidden className={cn("shrink-0", className)}>
      <defs>
        <linearGradient id={`${id}-sea`} x1="0.15" y1="0" x2="0.85" y2="1">
          <stop offset="0" stopColor="#86e2d8" />
          <stop offset="0.5" stopColor="#56b8de" />
          <stop offset="1" stopColor="#3a7cbf" />
        </linearGradient>
      </defs>
      <rect width="32" height="32" rx="8" fill={`url(#${id}-sea)`} />
      <path
        d={RIBBON}
        fill="none"
        stroke="#ffffff"
        strokeWidth="3.3"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
