import { Check, ChevronsUpDown, Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { cn } from "@/lib/cn";
import { Popover } from "./Popover";

export interface SelectOption<T extends string> {
  value: T;
  label: string;
  /** Secondary text shown on the right of the option. */
  detail?: string;
}

export function Select<T extends string>({
  value,
  options,
  onChange,
  placeholder = "Choose…",
  searchable = false,
  className,
  "aria-label": ariaLabel,
}: {
  value: T;
  options: SelectOption<T>[];
  onChange: (value: T) => void;
  placeholder?: string;
  searchable?: boolean;
  className?: string;
  "aria-label"?: string;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const button = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const selected = options.find((o) => o.value === value);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q ? options.filter((o) => o.label.toLowerCase().includes(q)) : options;
  }, [options, query]);

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setActive(Math.max(0, options.findIndex((o) => o.value === value)));
  }, [open, options, value]);

  useEffect(() => {
    list.current?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: "nearest" });
  }, [active, open]);

  const choose = (option?: SelectOption<T>) => {
    if (option) onChange(option.value);
    setOpen(false);
    button.current?.focus();
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      if (!open) return setOpen(true);
      setActive((i) => Math.min(filtered.length - 1, i + 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((i) => Math.max(0, i - 1));
    } else if (e.key === "Enter" && open) {
      e.preventDefault();
      choose(filtered[active]);
    }
  };

  return (
    <>
      <button
        ref={button}
        type="button"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={ariaLabel}
        onClick={() => setOpen((o) => !o)}
        onKeyDown={onKeyDown}
        className={cn(
          "flex h-9 w-full items-center gap-2 rounded-control border border-line bg-surface px-3 text-left",
          "transition-[border-color] duration-150 hover:border-line-strong",
          open && "border-accent",
          className,
        )}
      >
        <span className={cn("flex-1 truncate type-body", !selected && "text-ink-muted")}>
          {selected?.label ?? placeholder}
        </span>
        <ChevronsUpDown size={14} className="shrink-0 text-ink-muted" />
      </button>

      <Popover anchor={button} open={open} onClose={() => setOpen(false)}>
        {searchable && (
          <div className="flex items-center gap-2 border-b border-line-subtle px-3">
            <Search size={14} className="text-ink-muted" />
            <input
              autoFocus
              value={query}
              onChange={(e) => {
                setQuery(e.target.value);
                setActive(0);
              }}
              onKeyDown={onKeyDown}
              placeholder="Search"
              className="h-9 flex-1 bg-transparent type-body outline-none placeholder:text-ink-muted"
            />
          </div>
        )}
        <div ref={list} role="listbox" className="max-h-72 overflow-y-auto p-1">
          {filtered.length === 0 && <div className="px-3 py-2 type-small text-ink-muted">No matches</div>}
          {filtered.map((option, i) => (
            <div
              key={option.value}
              role="option"
              aria-selected={option.value === value}
              data-index={i}
              onPointerMove={() => setActive(i)}
              onClick={() => choose(option)}
              className={cn(
                "flex h-8 items-center gap-2 rounded-inner px-2.5 type-body",
                i === active && "bg-hover",
              )}
            >
              <Check
                size={14}
                strokeWidth={2.5}
                className={cn("shrink-0 text-accent-ink", option.value !== value && "invisible")}
              />
              <span className="flex-1 truncate">{option.label}</span>
              {option.detail && <span className="type-caption text-ink-muted">{option.detail}</span>}
            </div>
          ))}
        </div>
      </Popover>
    </>
  );
}
