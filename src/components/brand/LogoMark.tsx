import { cn } from "@/lib/cn";

const BARS = [0.35, 0.7, 1, 0.55, 0.85, 0.4];

/** The SpeakType mark: a neon waveform on a black rounded square, like the recorder pill. */
export function LogoMark({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 32 32" aria-hidden className={cn("shrink-0", className)}>
      <rect width="32" height="32" rx="8" fill="#0a0a0a" />
      {BARS.map((height, i) => {
        const h = height * 16;
        return <rect key={i} x={7.5 + i * 3.2} y={16 - h / 2} width="2" height={h} rx="1" fill="#39f27a" />;
      })}
    </svg>
  );
}
