import { cn } from "@/lib/cn";

/** `value` is 0..1. */
export function ProgressBar({ value, className }: { value: number; className?: string }) {
  return (
    <div
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(value * 100)}
      className={cn("h-1.5 overflow-hidden rounded-full bg-hover", className)}
    >
      <div
        className="h-full rounded-full bg-primary transition-[width] duration-300 ease-out-soft"
        style={{ width: `${Math.min(100, Math.max(0, value * 100))}%` }}
      />
    </div>
  );
}
