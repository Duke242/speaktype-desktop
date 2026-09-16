import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "@/lib/cn";
import type { Tone } from "./Badge";
import { Card } from "./Card";
import { IconTile } from "./IconTile";

/**
 * Scrollable page body. Every screen uses the same width so titles and cards
 * line up when switching between them.
 */
export function Page({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div className="h-full overflow-y-auto">
      <div className={cn("mx-auto max-w-[960px] px-10 pt-9 pb-14", className)}>{children}</div>
    </div>
  );
}

export function PageHeader({
  title,
  description,
  actions,
}: {
  title: string;
  description?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <header className="mb-7 flex items-end gap-6">
      <div className="min-w-0 flex-1">
        <h1 className="type-title">{title}</h1>
        {description && <p className="mt-1 type-body text-ink-secondary">{description}</p>}
      </div>
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </header>
  );
}

/** A titled group of rows inside one card. */
export function Section({
  title,
  description,
  children,
  className,
}: {
  title?: string;
  description?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={cn("mb-8", className)}>
      {title && (
        <div className="mb-3 px-1">
          <h2 className="type-section">{title}</h2>
          {description && <p className="mt-0.5 type-small text-ink-secondary">{description}</p>}
        </div>
      )}
      <Card padding="none" className="divide-y divide-line-subtle">
        {children}
      </Card>
    </section>
  );
}

/** One setting: label and description on the left, control on the right. */
export function SettingRow({
  icon,
  tone,
  label,
  description,
  children,
}: {
  icon?: LucideIcon;
  tone?: Tone;
  label: string;
  description?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className="flex items-center gap-4 px-5 py-4">
      <div className="flex min-w-0 flex-1 items-center gap-3.5">
        {icon && <IconTile icon={icon} tone={tone} />}
        <div className="min-w-0">
          <div className="type-label text-ink">{label}</div>
          {description && <div className="mt-0.5 type-small text-ink-secondary">{description}</div>}
        </div>
      </div>
      {children && <div className="shrink-0">{children}</div>}
    </div>
  );
}

export function EmptyState({
  icon,
  title,
  description,
  action,
  className,
}: {
  icon: LucideIcon;
  title: string;
  description?: ReactNode;
  action?: ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("flex flex-col items-center px-6 py-14 text-center", className)}>
      <IconTile icon={icon} tone="brand" size="lg" className="mb-4" />
      <h3 className="type-section">{title}</h3>
      {description && <p className="mt-1 max-w-sm type-small text-ink-secondary">{description}</p>}
      {action && <div className="mt-5">{action}</div>}
    </div>
  );
}

/** A small inline message, e.g. a setup warning. */
export function Callout({
  icon,
  tone = "warning",
  title,
  children,
  action,
}: {
  icon: LucideIcon;
  tone?: Tone;
  title?: string;
  children: ReactNode;
  action?: ReactNode;
}) {
  return (
    <Card padding="sm" className="flex items-center gap-3.5">
      <IconTile icon={icon} tone={tone} />
      <div className="min-w-0 flex-1">
        {title && <div className="type-label">{title}</div>}
        <div className="type-small text-ink-secondary">{children}</div>
      </div>
      {action && <div className="shrink-0">{action}</div>}
    </Card>
  );
}
