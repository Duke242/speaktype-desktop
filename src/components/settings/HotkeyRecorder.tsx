import { useEffect, useRef, useState } from "react";
import { Hotkey } from "@/components/ui";
import { cn } from "@/lib/cn";
import { acceleratorFrom, heldModifiers } from "@/lib/hotkeys";

export function HotkeyRecorder({ value, onChange }: { value: string; onChange: (hotkey: string) => void }) {
  const [recording, setRecording] = useState(false);
  const [preview, setPreview] = useState("");
  const button = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!recording) return;
    const onKeyDown = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      if (event.code === "Escape") return setRecording(false);
      const accelerator = acceleratorFrom(event);
      if (!accelerator) return setPreview(heldModifiers(event));
      setRecording(false);
      if (accelerator !== value) onChange(accelerator);
    };
    const onKeyUp = (event: KeyboardEvent) => setPreview(heldModifiers(event));
    const stop = () => setRecording(false);
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("keyup", onKeyUp, true);
    window.addEventListener("blur", stop);
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("keyup", onKeyUp, true);
      window.removeEventListener("blur", stop);
    };
  }, [recording, value, onChange]);

  return (
    <button
      ref={button}
      type="button"
      onClick={() => {
        setPreview("");
        setRecording((r) => !r);
      }}
      onBlur={() => setRecording(false)}
      className={cn(
        "flex h-9 min-w-[180px] items-center justify-center gap-2 rounded-control border px-3 transition-colors",
        recording ? "border-accent bg-accent-soft" : "border-line bg-surface hover:border-line-strong",
      )}
    >
      {recording ? (
        preview ? (
          <Hotkey value={preview} />
        ) : (
          <span className="type-small text-accent-ink">Press a shortcut…</span>
        )
      ) : (
        <Hotkey value={value} />
      )}
    </button>
  );
}
