import { cn } from "@/lib/cn";

const BARS = [0.35, 0.6, 0.9, 0.55, 1, 0.7, 0.4, 0.85, 0.5, 0.95, 0.65, 0.3, 0.75, 0.55, 0.9, 0.45];

/** A decorative, animated copy of the recorder pill, used to show what dictation looks like. */
export function PillPreview({ className }: { className?: string }) {
  return (
    <div
      aria-hidden
      className={cn(
        "flex h-12 items-center gap-3 rounded-full bg-pill px-4 shadow-elevated ring-1 ring-white/10",
        className,
      )}
    >
      <span className="relative flex size-2.5">
        <span className="absolute inset-0 animate-ping rounded-full bg-recording/40" />
        <span className="relative size-2.5 rounded-full bg-recording" />
      </span>
      <div className="flex h-6 items-center gap-[3px]">
        {BARS.map((height, i) => (
          <span
            key={i}
            className="w-[3px] origin-center animate-wave rounded-full bg-white/90"
            style={{ height: `${height * 100}%`, animationDelay: `${(i % 7) * -0.13}s` }}
          />
        ))}
      </div>
      <span className="type-caption font-medium text-white/60 tabular-nums">0:03</span>
    </div>
  );
}
