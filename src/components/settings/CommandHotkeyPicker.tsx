import { X } from "lucide-react";
import { Button, Hotkey, IconButton, useToast } from "@/components/ui";
import { errorMessage } from "@/lib/api";
import { useStore } from "@/lib/store";
import { HotkeyRecorder } from "./HotkeyRecorder";

/**
 * The suggested command shortcut. Ctrl+Shift+E is free on a Mac but toggles
 * Track Changes in Word elsewhere, and Ctrl+Alt+Space would switch input
 * sources on a Mac.
 */
function recommendedCommandHotkey(os: string) {
  return os === "macos" ? "Ctrl+Shift+E" : "Ctrl+Alt+Space";
}

/** Records the command mode shortcut, or turns it off. */
export function CommandHotkeyPicker() {
  const { settings, status, updateSettings } = useStore();
  const toast = useToast();
  const save = (commandHotkey: string) =>
    updateSettings({ commandHotkey }).catch((e) => toast(errorMessage(e), "error"));
  const recommended = recommendedCommandHotkey(status.os);

  return (
    <div className="flex items-center gap-1.5">
      {!settings.commandHotkey && recommended !== settings.hotkey && (
        <Button size="sm" variant="ghost" onClick={() => save(recommended)}>
          Use <Hotkey value={recommended} /> (Recommended)
        </Button>
      )}
      <HotkeyRecorder value={settings.commandHotkey} onChange={save} placeholder="Click to set a shortcut" />
      {settings.commandHotkey && <IconButton icon={X} label="Turn off" size="sm" onClick={() => save("")} />}
    </div>
  );
}
