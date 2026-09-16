import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/cn";

export interface Segment<T extends string> {
  value: T;
  label: string;
  icon?: LucideIcon;
}

export function SegmentedControl<T extends string>({
  value,
  options,
  onChange,
  className,
}: {
  value: T;
  options: Segment<T>[];
  onChange: (value: T) => void;
  className?: string;
}) {
  return (
    <div role="radiogroup" className={cn("inline-flex h-9 rounded-control bg-hover p-[3px]", className)}>
      {options.map(({ value: v, label, icon: Icon }) => {
        const selected = v === value;
        return (
          <button
            key={v}
            type="button"
            role="radio"
            aria-checked={selected}
            onClick={() => onChange(v)}
            className={cn(
              "inline-flex flex-1 items-center justify-center gap-1.5 whitespace-nowrap rounded-inner px-3 type-small font-medium transition-colors duration-150",
              selected ? "bg-surface text-ink ring-1 ring-line" : "text-ink-secondary hover:text-ink",
            )}
          >
            {Icon && <Icon size={14} strokeWidth={2} />}
            {label}
          </button>
        );
      })}
    </div>
  );
}
