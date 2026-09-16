import type { LucideIcon } from "lucide-react";
import { forwardRef, type ButtonHTMLAttributes } from "react";
import { cn } from "@/lib/cn";
import { Spinner } from "./Spinner";

export type ButtonVariant = "accent" | "primary" | "secondary" | "ghost" | "danger";
export type ButtonSize = "sm" | "md" | "lg";

const variants: Record<ButtonVariant, string> = {
  // Neon green, for the one main action on a screen.
  accent: "bg-accent text-on-accent hover:bg-accent-strong",
  primary: "bg-primary text-on-primary hover:bg-primary-hover",
  secondary: "border border-line bg-surface text-ink hover:border-line-strong hover:bg-hover",
  ghost: "text-ink-secondary hover:bg-hover hover:text-ink",
  danger: "text-danger hover:bg-danger-soft",
};

const sizes: Record<ButtonSize, string> = {
  sm: "h-8 gap-1.5 px-3 type-small font-medium",
  md: "h-9 gap-2 px-3.5 type-label",
  lg: "h-11 gap-2 px-5 type-label",
};

const iconSizes: Record<ButtonSize, number> = { sm: 14, md: 16, lg: 16 };

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  icon?: LucideIcon;
  loading?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "secondary", size = "md", icon: Icon, loading, disabled, className, children, ...props },
  ref,
) {
  return (
    <button
      ref={ref}
      type="button"
      disabled={disabled || loading}
      className={cn(
        "inline-flex shrink-0 items-center justify-center whitespace-nowrap rounded-control",
        "transition-[background-color,border-color,color,transform] duration-150 active:scale-[0.98]",
        "disabled:pointer-events-none disabled:opacity-45",
        variants[variant],
        sizes[size],
        className,
      )}
      {...props}
    >
      {loading ? <Spinner size={iconSizes[size] - 2} /> : Icon && <Icon size={iconSizes[size]} strokeWidth={2} />}
      {children}
    </button>
  );
});

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  icon: LucideIcon;
  label: string;
  size?: ButtonSize;
  tone?: "default" | "danger";
}

/** Square ghost button with an icon. `label` is used for the tooltip and screen readers. */
export function IconButton({ icon: Icon, label, size = "md", tone = "default", className, ...props }: IconButtonProps) {
  const box = { sm: "size-8", md: "size-9", lg: "size-11" }[size];
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className={cn(
        "inline-flex shrink-0 items-center justify-center rounded-control transition-colors duration-150",
        "disabled:pointer-events-none disabled:opacity-45",
        tone === "danger"
          ? "text-ink-muted hover:bg-danger-soft hover:text-danger"
          : "text-ink-muted hover:bg-hover hover:text-ink",
        box,
        className,
      )}
      {...props}
    >
      <Icon size={iconSizes[size]} strokeWidth={2} />
    </button>
  );
}
