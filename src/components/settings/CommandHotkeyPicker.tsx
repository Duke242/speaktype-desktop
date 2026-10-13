import { X } from "lucide-react";
import { IconButton, useToast } from "@/components/ui";
import { errorMessage } from "@/lib/api";
import { useStore } from "@/lib/store";
import { HotkeyRecorder } from "./HotkeyRecorder";

/** Records the command mode shortcut, or turns it off. */
export function CommandHotkeyPicker() {
  const { settings, updateSettings } = useStore();
  const toast = useToast();
  const save = (commandHotkey: string) =>
    updateSettings({ commandHotkey }).catch((e) => toast(errorMessage(e), "error"));

  return (
    <div className="flex items-center gap-1.5">
      <HotkeyRecorder value={settings.commandHotkey} onChange={save} placeholder="Click to set a shortcut" />
      {settings.commandHotkey && <IconButton icon={X} label="Turn off" size="sm" onClick={() => save("")} />}
    </div>
  );
}
