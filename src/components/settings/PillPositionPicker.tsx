import type { PillPosition } from "@/lib/api";
import { cn } from "@/lib/cn";

const POSITIONS: { value: PillPosition; label: string }[] = [
  { value: "topLeft", label: "Top left" },
  { value: "topCenter", label: "Top center" },
  { value: "topRight", label: "Top right" },
  { value: "centerLeft", label: "Middle left" },
  { value: "center", label: "Center" },
  { value: "centerRight", label: "Middle right" },
  { value: "bottomLeft", label: "Bottom left" },
  { value: "bottomCenter", label: "Bottom center" },
  { value: "bottomRight", label: "Bottom right" },
];

export function pillPositionLabel(position: PillPosition) {
  return POSITIONS.find((p) => p.value === position)?.label ?? "";
}

/** A miniature screen with nine spots for the recorder pill. */
export function PillPositionPicker({
  value,
  onChange,
}: {
  value: PillPosition;
  onChange: (position: PillPosition) => void;
}) {
  return (
    <div
      role="radiogroup"
      aria-label="Recorder pill position"
      className="grid h-[104px] w-[168px] grid-cols-3 grid-rows-3 gap-1 rounded-control border border-line bg-surface-sunken p-1.5"
    >
      {POSITIONS.map((position) => {
        const selected = position.value === value;
        return (
          <button
            key={position.value}
            type="button"
            role="radio"
            aria-checked={selected}
            aria-label={position.label}
            title={position.label}
            onClick={() => onChange(position.value)}
            className="group flex items-center justify-center rounded-inner transition-colors hover:bg-hover"
          >
            <span
              className={cn(
                "h-2 rounded-full transition-all duration-200 ease-out-soft",
                selected ? "w-7 bg-primary" : "w-4 bg-line-strong group-hover:bg-ink-muted",
              )}
            />
          </button>
        );
      })}
    </div>
  );
}
