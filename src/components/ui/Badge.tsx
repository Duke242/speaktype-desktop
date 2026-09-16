import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "@/lib/cn";

export type Tone = "brand" | "neutral" | "accent" | "info" | "success" | "warning" | "danger";

const tones: Record<Tone, string> = {
  brand: "bg-primary text-on-primary",
  neutral: "bg-hover text-ink-secondary",
  accent: "bg-accent-soft text-accent-ink",
  success: "bg-success-soft text-success",
  warning: "bg-warning-soft text-warning",
  danger: "bg-danger-soft text-danger",
  info: "bg-info-soft text-info",
};

export function Badge({
  tone = "neutral",
  icon: Icon,
  children,
  className,
}: {
  tone?: Tone;
  icon?: LucideIcon;
  children: ReactNode;
  className?: string;
}) {
  return (
    <span
      className={cn(
        "inline-flex h-5 shrink-0 items-center gap-1 whitespace-nowrap rounded-full px-2 type-caption font-medium",
        tones[tone],
        className,
      )}
    >
      {Icon && <Icon size={12} strokeWidth={2.25} />}
      {children}
    </span>
  );
}
