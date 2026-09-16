import { cn } from "@/lib/cn";
import { hotkeyParts } from "@/lib/format";

export function Kbd({ children, className }: { children: string; className?: string }) {
  return (
    <kbd
      className={cn(
        "inline-flex h-6 min-w-6 items-center justify-center rounded-inner border border-line border-b-2 bg-surface px-1.5",
        "font-sans type-caption font-medium text-ink",
        className,
      )}
    >
      {children}
    </kbd>
  );
}

/** Renders a hotkey like "Ctrl+Shift+Space" as separate keycaps. */
export function Hotkey({ value, className }: { value: string; className?: string }) {
  return (
    <span className={cn("inline-flex items-center gap-1 align-middle", className)}>
      {hotkeyParts(value).map((part, i) => (
        <Kbd key={i}>{part}</Kbd>
      ))}
    </span>
  );
}
