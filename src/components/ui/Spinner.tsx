import { cn } from "@/lib/cn";

/** A ring with a fading tail, matching the macOS app's spinner. Inherits text color. */
export function Spinner({ size = 14, className }: { size?: number; className?: string }) {
  const stroke = Math.max(1.5, size / 7);
  return (
    <span
      role="status"
      aria-label="Loading"
      className={cn("inline-block shrink-0 animate-spin-fast rounded-full", className)}
      style={{
        width: size,
        height: size,
        background: "conic-gradient(from 0deg, transparent 0deg, currentColor 295deg, transparent 296deg)",
        mask: `radial-gradient(farthest-side, transparent calc(100% - ${stroke}px), #000 calc(100% - ${stroke}px + 0.5px))`,
        WebkitMask: `radial-gradient(farthest-side, transparent calc(100% - ${stroke}px), #000 calc(100% - ${stroke}px + 0.5px))`,
      }}
    />
  );
}
