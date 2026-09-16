import { cn } from "@/lib/cn";

export function Switch({
  checked,
  onChange,
  disabled,
  label,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  /** Accessible name when there is no visible label next to it. */
  label?: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn(
        "relative inline-flex h-[22px] w-[38px] shrink-0 items-center rounded-full transition-colors duration-200 disabled:opacity-45",
        checked ? "bg-primary" : "bg-line-strong",
      )}
    >
      <span
        className={cn(
          "absolute left-[3px] size-4 rounded-full shadow-elevated transition-[transform,background-color] duration-200 ease-out-soft",
          checked ? "translate-x-4 bg-accent" : "bg-white",
        )}
      />
    </button>
  );
}
