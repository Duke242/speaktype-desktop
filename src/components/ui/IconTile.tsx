import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/cn";
import type { Tone } from "./Badge";

const tones: Record<Tone, string> = {
  brand: "bg-primary text-accent dark:bg-surface-sunken dark:ring-1 dark:ring-line",
  neutral: "bg-hover text-ink-secondary",
  accent: "bg-accent-soft text-accent-ink",
  success: "bg-success-soft text-success",
  warning: "bg-warning-soft text-warning",
  danger: "bg-danger-soft text-danger",
  info: "bg-info-soft text-info",
};

const sizes = {
  sm: { box: "size-8 rounded-control", icon: 16 },
  md: { box: "size-10 rounded-control", icon: 18 },
  lg: { box: "size-12 rounded-card", icon: 22 },
};

/** The one tinted icon square used beside titles, stats and settings. `brand` is neon on black, for hero moments. */
export function IconTile({
  icon: Icon,
  tone = "neutral",
  size = "sm",
  className,
}: {
  icon: LucideIcon;
  tone?: Tone;
  size?: keyof typeof sizes;
  className?: string;
}) {
  return (
    <div className={cn("flex shrink-0 items-center justify-center", tones[tone], sizes[size].box, className)}>
      <Icon size={sizes[size].icon} strokeWidth={2} />
    </div>
  );
}
