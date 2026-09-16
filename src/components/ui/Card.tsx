import type { HTMLAttributes } from "react";
import { cn } from "@/lib/cn";

type Padding = "none" | "sm" | "md" | "lg";

const paddings: Record<Padding, string> = { none: "", sm: "p-4", md: "p-5", lg: "p-6" };

export interface CardProps extends HTMLAttributes<HTMLDivElement> {
  padding?: Padding;
  interactive?: boolean;
}

/** The one card style: white surface, hairline border, a barely-there lift, 12px radius. */
export function Card({ padding = "md", interactive, className, ...props }: CardProps) {
  return (
    <div
      className={cn(
        "rounded-card border border-line bg-surface shadow-card",
        interactive && "transition-colors duration-150 hover:border-line-strong",
        paddings[padding],
        className,
      )}
      {...props}
    />
  );
}
