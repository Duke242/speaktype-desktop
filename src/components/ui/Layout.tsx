import type { LucideIcon } from "lucide-react";
import { type PointerEvent as ReactPointerEvent, type ReactNode, type RefObject, useEffect, useRef, useState } from "react";
import { cn } from "@/lib/cn";
import type { Tone } from "./Badge";
import { IconTile } from "./IconTile";

const widths = {
  /** Lists and settings: a comfortable line length for reading. */
  reading: "max-w-[800px]",
  /** Charts and tables that need a little more room. */
  wide: "max-w-[960px]",
};

/**
 * Scrollable page body: one centered column, so the eye lands in the same
 * place on every screen. The native scrollbar is hidden and a thin one floats
 * over the content instead, so pages never shift sideways and the bar stays
 * clear of the window's rounded corners.
 */
export function Page({
  children,
  width = "reading",
  className,
}: {
  children: ReactNode;
  width?: keyof typeof widths;
  className?: string;
}) {
  const viewport = useRef<HTMLDivElement>(null);
  return (
    <div className="relative h-full">
      <div ref={viewport} className="no-scrollbar h-full overflow-y-auto">
        <div className={cn("@container mx-auto px-10 pt-14 pb-16", widths[width], className)}>{children}</div>
      </div>
      <ScrollThumb viewport={viewport} />
    </div>
  );
}

/** Space kept between the scrollbar and the top and bottom of the window. */
const TRACK_INSET = 14;
const MIN_THUMB = 36;

function ScrollThumb({ viewport }: { viewport: RefObject<HTMLDivElement | null> }) {
  const [thumb, setThumb] = useState<{ top: number; height: number } | null>(null);
  const [scrolling, setScrolling] = useState(false);
  const [dragging, setDragging] = useState(false);
  const drag = useRef<{ pointerY: number; scrollTop: number } | null>(null);

  useEffect(() => {
    const el = viewport.current;
    if (!el) return;
    let idle: ReturnType<typeof setTimeout> | undefined;

    const measure = () => {
      const track = el.clientHeight - TRACK_INSET * 2;
      const scrollable = el.scrollHeight - el.clientHeight;
      if (scrollable <= 1 || track <= MIN_THUMB) return setThumb(null);
      const height = Math.max(MIN_THUMB, (el.clientHeight / el.scrollHeight) * track);
      const top = TRACK_INSET + (el.scrollTop / scrollable) * (track - height);
      setThumb({ top, height });
    };
    const onScroll = () => {
      measure();
      setScrolling(true);
      clearTimeout(idle);
      idle = setTimeout(() => setScrolling(false), 900);
    };

    measure();
    el.addEventListener("scroll", onScroll, { passive: true });
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    if (el.firstElementChild) observer.observe(el.firstElementChild);
    return () => {
      el.removeEventListener("scroll", onScroll);
      observer.disconnect();
      clearTimeout(idle);
    };
  }, [viewport]);

  if (!thumb) return null;

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    const el = viewport.current;
    if (!el) return;
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { pointerY: e.clientY, scrollTop: el.scrollTop };
    setDragging(true);
  };
  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const el = viewport.current;
    if (!el || !drag.current) return;
    const track = el.clientHeight - TRACK_INSET * 2;
    const ratio = (el.scrollHeight - el.clientHeight) / Math.max(1, track - thumb.height);
    el.scrollTop = drag.current.scrollTop + (e.clientY - drag.current.pointerY) * ratio;
  };
  const onPointerUp = () => {
    drag.current = null;
    setDragging(false);
  };

  return (
    <div
      aria-hidden
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      style={{ top: thumb.top, height: thumb.height }}
      className={cn(
        "group/thumb absolute right-0.5 flex w-3 justify-center opacity-0 transition-opacity duration-300 hover:opacity-100",
        (scrolling || dragging) && "opacity-100",
      )}
    >
      <div
        className={cn(
          "h-full w-1.5 rounded-full bg-scrollbar transition-[width,background-color] duration-150",
          "group-hover/thumb:w-2 group-hover/thumb:bg-scrollbar-active",
          dragging && "w-2 bg-scrollbar-active",
        )}
      />
    </div>
  );
}

/** A big, quiet page title with an optional line under it and actions to the right. */
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
    <header className="mb-10 flex items-start gap-6">
      <div className="min-w-0 flex-1">
        <h1 className="type-title">{title}</h1>
        {description && <p className="mt-2 max-w-[560px] type-body-lg text-ink-secondary">{description}</p>}
      </div>
      {actions && <div className="flex h-10 shrink-0 items-center gap-2">{actions}</div>}
    </header>
  );
}

/** The small uppercase label over a list, on a hairline, with an optional link or action at the end. */
export function GroupHeader({
  title,
  description,
  action,
  className,
}: {
  title: ReactNode;
  description?: ReactNode;
  action?: ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("flex min-h-8 items-end gap-4 border-b border-line pb-3", className)}>
      <div className="min-w-0 flex-1">
        <h2 className="type-overline text-ink-muted">{title}</h2>
        {description && <p className="mt-1 type-small text-ink-secondary">{description}</p>}
      </div>
      {action && <div className="-mb-1 shrink-0">{action}</div>}
    </div>
  );
}

/** A titled group of rows, separated by soft hairlines rather than boxed in a card. */
export function Section({
  title,
  description,
  action,
  children,
  className,
}: {
  title?: string;
  description?: ReactNode;
  action?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={cn("mb-12", className)}>
      {title && <GroupHeader title={title} description={description} action={action} />}
      <div className="divide-y divide-line">{children}</div>
    </section>
  );
}

/** One setting: label and description on the left, control on the right. */
export function SettingRow({
  icon: Icon,
  label,
  description,
  children,
}: {
  icon?: LucideIcon;
  label: string;
  description?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className="flex items-center gap-6 py-4">
      <div className="flex min-w-0 flex-1 items-start gap-3.5">
        {Icon && <Icon size={18} strokeWidth={1.75} className="mt-px shrink-0 text-ink-muted" />}
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
    <div className={cn("flex flex-col items-center px-6 py-16 text-center", className)}>
      <IconTile icon={icon} tone="accent" size="lg" className="mb-5" />
      <h3 className="type-section">{title}</h3>
      {description && <p className="mt-1.5 max-w-sm type-body text-ink-secondary">{description}</p>}
      {action && <div className="mt-6">{action}</div>}
    </div>
  );
}

const calloutTones: Record<Tone, string> = {
  brand: "bg-accent-soft text-accent-ink",
  accent: "bg-accent-soft text-accent-ink",
  neutral: "bg-hover/70 text-ink-secondary",
  info: "bg-info-soft text-info",
  success: "bg-success-soft text-success",
  warning: "bg-warning-soft text-warning",
  danger: "bg-danger-soft text-danger",
};

/** A small inline message on a soft tint, e.g. a setup warning. */
export function Callout({
  icon: Icon,
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
    <div className={cn("flex items-center gap-3.5 rounded-card px-4 py-3", calloutTones[tone])}>
      <Icon size={18} strokeWidth={2} className="shrink-0" />
      <div className="min-w-0 flex-1">
        {title && <div className="type-label text-ink">{title}</div>}
        <div className="type-small text-ink-secondary">{children}</div>
      </div>
      {action && <div className="shrink-0">{action}</div>}
    </div>
  );
}
