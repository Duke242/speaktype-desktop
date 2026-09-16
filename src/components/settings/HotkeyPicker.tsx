import { useToast } from "@/components/ui";
import { Select, type SelectOption } from "@/components/ui";
import { errorMessage } from "@/lib/api";
import { MODIFIER_HOTKEY_LABELS } from "@/lib/format";
import { useStore } from "@/lib/store";
import { HotkeyRecorder } from "./HotkeyRecorder";

const CUSTOM = "custom";

/**
 * Chooses the dictation hotkey: a single modifier key where the OS supports
 * it (Fn on a Mac), or any shortcut the user records.
 */
export function HotkeyPicker() {
  const { settings, status, updateSettings } = useStore();
  const toast = useToast();
  const save = (hotkey: string) => updateSettings({ hotkey }).catch((e) => toast(errorMessage(e), "error"));

  const modifiers = status.modifierHotkeys;
  const isModifier = modifiers.includes(settings.hotkey);

  if (modifiers.length === 0) {
    return <HotkeyRecorder value={settings.hotkey} onChange={save} />;
  }

  const options: SelectOption<string>[] = [
    ...modifiers.map((key) => ({
      value: key,
      label: MODIFIER_HOTKEY_LABELS[key] === "fn" ? "fn (Globe) key" : `${MODIFIER_HOTKEY_LABELS[key]} key`,
      detail: key === "Fn" ? "Recommended" : undefined,
    })),
    { value: CUSTOM, label: "Custom shortcut…" },
  ];

  return (
    <div className="flex items-center gap-2">
      <Select
        value={isModifier ? settings.hotkey : CUSTOM}
        options={options}
        onChange={(value) => {
          // Keep the current custom shortcut until the user records a new one.
          if (value !== CUSTOM) save(value);
          else if (isModifier) save("Ctrl+Shift+Space");
        }}
        aria-label="Hotkey"
        className="w-[200px]"
      />
      {!isModifier && <HotkeyRecorder value={settings.hotkey} onChange={save} />}
    </div>
  );
}
