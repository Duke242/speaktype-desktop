import { useEffect, useRef, useState } from "react";
import { Hotkey } from "@/components/ui";
import { cn } from "@/lib/cn";

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "ShiftLeft",
  "ShiftRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
]);

/**
 * Builds an accelerator such as "Ctrl+Shift+Space" from a key press.
 * KeyboardEvent.code names ("KeyD", "Digit1", "F5") are what the Rust side parses.
 */
function acceleratorFrom(event: KeyboardEvent): string | null {
  if (MODIFIER_CODES.has(event.code)) return null;
  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  if (event.metaKey) parts.push("Super");
  const key = event.code.replace(/^Key/, "").replace(/^Digit/, "");
  // A bare letter would fire while typing, so require a modifier unless it's a function key.
  if (parts.length === 0 && !/^F\d+$/.test(key)) return null;
  return [...parts, key].join("+");
}

function heldModifiers(event: KeyboardEvent) {
  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  if (event.metaKey) parts.push("Super");
  return parts.join("+");
}

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
