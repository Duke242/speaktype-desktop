import { useEffect, useLayoutEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import { createPortal } from "react-dom";
import { cn } from "@/lib/cn";

/**
 * A floating panel anchored below (or above, if there's no room) an element.
 * Rendered in a portal so scrolling containers don't clip it.
 */
export function Popover({
  anchor,
  open,
  onClose,
  children,
  matchWidth = true,
  align = "start",
  className,
}: {
  anchor: RefObject<HTMLElement | null>;
  open: boolean;
  onClose: () => void;
  children: ReactNode;
  matchWidth?: boolean;
  align?: "start" | "end";
  className?: string;
}) {
  const panel = useRef<HTMLDivElement>(null);
  const [style, setStyle] = useState<React.CSSProperties>({ visibility: "hidden" });

  useLayoutEffect(() => {
    if (!open || !anchor.current || !panel.current) return;
    const rect = anchor.current.getBoundingClientRect();
    const height = panel.current.offsetHeight;
    const width = panel.current.offsetWidth;
    const gap = 6;
    const below = rect.bottom + gap + height <= window.innerHeight - 8;
    const top = below ? rect.bottom + gap : Math.max(8, rect.top - gap - height);
    const left =
      align === "end"
        ? Math.max(8, rect.right - Math.max(width, matchWidth ? rect.width : 0))
        : Math.min(rect.left, window.innerWidth - Math.max(width, rect.width) - 8);
    setStyle({ top, left, minWidth: matchWidth ? rect.width : undefined });
  }, [open, anchor, matchWidth, align]);

  useEffect(() => {
    if (!open) return;
    const onPointer = (e: PointerEvent) => {
      const target = e.target as Node;
      if (!panel.current?.contains(target) && !anchor.current?.contains(target)) onClose();
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    const onBlur = () => onClose();
    document.addEventListener("pointerdown", onPointer, true);
    document.addEventListener("keydown", onKey);
    window.addEventListener("blur", onBlur);
    window.addEventListener("resize", onBlur);
    return () => {
      document.removeEventListener("pointerdown", onPointer, true);
      document.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("resize", onBlur);
    };
  }, [open, onClose, anchor]);

  if (!open) return null;
  return createPortal(
    <div
      ref={panel}
      style={style}
      className={cn(
        "fixed z-50 animate-pop-in overflow-hidden rounded-card border border-line bg-surface shadow-elevated",
        className,
      )}
    >
      {children}
    </div>,
    document.body,
  );
}
