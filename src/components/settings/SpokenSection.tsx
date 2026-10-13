import { Pilcrow, Undo2 } from "lucide-react";
import { Section, SettingRow, Switch, useToast } from "@/components/ui";
import { errorMessage, type Settings } from "@/lib/api";
import { useStore } from "@/lib/store";

/** Offline rules for the way people talk: commands, corrections and formatting. */
export function SpokenSection() {
  const { settings, updateSettings } = useStore();
  const toast = useToast();
  const save = (patch: Partial<Settings>) => updateSettings(patch).catch((e) => toast(errorMessage(e), "error"));
  const english = settings.language === "auto" || settings.language.startsWith("en");

  return (
    <Section
      title="Commands and formatting"
      description={
        english
          ? "Offline rules for English dictation."
          : "Offline rules for English dictation. They're off while your spoken language is set to another language."
      }
    >
      <SettingRow
        icon={Pilcrow}
        label="Voice commands"
        description="Say “scratch that” to take back the last sentence, or “new line” and “new paragraph” to start one. Off while typing live."
      >
        <Switch checked={settings.voiceCommands} onChange={(voiceCommands) => save({ voiceCommands })} />
      </SettingRow>
      <SettingRow
        icon={Undo2}
        label="Keep only my correction"
        description="When you correct yourself mid-sentence, the correction replaces what it fixes: “meet at 3, actually 4” becomes “meet at 4”."
      >
        <Switch checked={settings.selfCorrections} onChange={(selfCorrections) => save({ selfCorrections })} />
      </SettingRow>
    </Section>
  );
}
