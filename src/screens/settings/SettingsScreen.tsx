import {
  AudioLines,
  BookA,
  ClipboardCheck,
  Command,
  CornerDownLeft,
  Ear,
  Globe,
  Hand,
  Import,
  Keyboard,
  Lightbulb,
  Mic,
  Monitor,
  Music,
  Power,
  Moon,
  PanelBottom,
  RefreshCw,
  RotateCw,
  Shield,

  Sparkles,
  Sun,
  TextCursorInput,
  Wand2,
  type LucideIcon,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { AppToneSection } from "@/components/settings/AppToneSection";
import { CleanupSection } from "@/components/settings/CleanupSection";
import { CommandHotkeyPicker } from "@/components/settings/CommandHotkeyPicker";
import { HotkeyPicker } from "@/components/settings/HotkeyPicker";
import { PillPositionPicker } from "@/components/settings/PillPositionPicker";
import { SharingSection } from "@/components/settings/SharingSection";
import { SpokenSection } from "@/components/settings/SpokenSection";
import { PermissionList } from "@/components/PermissionList";
import { UpdateDialog } from "@/components/UpdateDialog";
import {
  Badge,
  Button,
  EmptyState,
  Page,
  PageHeader,
  Section,
  SegmentedControl,
  Select,
  SettingRow,
  Switch,
  useToast,
  type SelectOption,
} from "@/components/ui";
import { api, errorMessage, type InputDevice, type LegacyStatus, type Settings, type UpdateInfo } from "@/lib/api";
import { cn } from "@/lib/cn";
import { describeImport, hotkeyParts } from "@/lib/format";
import { hotkeyBlockedMessage, useWhileHotkeyBlocked } from "@/lib/hotkeyBlocked";

import { LANGUAGES, languageName } from "@/lib/languages";
import { useStore } from "@/lib/store";

type Tab = "general" | "audio" | "permissions";

export function SettingsScreen() {
  const { status } = useStore();
  const [tab, setTab] = useState<Tab>("general");
  const tabs: { value: Tab; label: string; icon: LucideIcon }[] = [
    { value: "general", label: "General", icon: Command },
    { value: "audio", label: "Audio", icon: Mic },
  ];
  // Only macOS has per-app permissions to manage.
  if (status.os === "macos") tabs.push({ value: "permissions", label: "Permissions", icon: Shield });

  return (
    <Page>
      <PageHeader
        title="Settings"
        actions={<SegmentedControl value={tab} options={tabs} onChange={setTab} className={tabs.length > 2 ? "w-[340px]" : "w-[230px]"} />}
      />
      {tab === "general" && <GeneralTab />}
      {tab === "audio" && (
        <>
          <AudioTab />
          <RecordingSection />
        </>
      )}
      {tab === "permissions" && <PermissionsTab />}
    </Page>
  );
}

/** Saves a settings change and shows a toast if the backend rejects it. */
function useSave() {
  const { updateSettings } = useStore();
  const toast = useToast();
  return useCallback(
    (patch: Partial<Settings>) => updateSettings(patch).catch((e) => toast(errorMessage(e), "error")),
    [updateSettings, toast],
  );
}

function GeneralTab() {
  const { settings, status, refreshStatus } = useStore();
  const save = useSave();
  useWhileHotkeyBlocked(status.hotkeyBlocked, refreshStatus);
  const isMac = status.os === "macos";

  return (
    <>
      <Section title="Appearance">
        <SettingRow icon={Sun} label="Theme" description="Follow your system, or pick a look.">
          <SegmentedControl
            value={settings.theme}
            onChange={(theme) => save({ theme })}
            options={[
              { value: "system", label: "System", icon: Monitor },
              { value: "light", label: "Light", icon: Sun },
              { value: "dark", label: "Dark", icon: Moon },
            ]}
          />
        </SettingRow>
      </Section>

      <Section title="Shortcuts">
        <SettingRow
          icon={Keyboard}
         
          label="Dictation hotkey"
          description={isMac ? "Works in every app. Fn is the easiest to reach." : "Works in every app. Click to record a new one."}
        >
          <HotkeyPicker />
        </SettingRow>
        {status.hotkeyBlocked && (
          <div className="flex items-center justify-between gap-4 py-3 pl-8 type-small text-ink-secondary">
            <p>{hotkeyBlockedMessage(hotkeyParts(settings.hotkey).join(" + "))}</p>
            <Button size="sm" variant="ghost" onClick={() => api.openPermissionSettings("accessibility")}>
              Open Accessibility
            </Button>
          </div>
        )}
        <SettingRow
          icon={Hand}
         
          label="Recording mode"
          description={
            {
              hold: "Hold the hotkey while you talk, and let go when you're done. Double-tap it to keep recording hands-free, then press it once to stop.",
              toggle: "Press the hotkey to start recording, and press it again to stop.",
              hybrid: "Hold the hotkey while you talk, or tap it to keep recording hands-free and press it again to stop.",
            }[settings.recordingMode]
          }
        >
          <SegmentedControl
            value={settings.recordingMode}
            onChange={(recordingMode) => save({ recordingMode })}
            options={[
              { value: "hold", label: "Hold to talk" },
              { value: "toggle", label: "Toggle" },
              { value: "hybrid", label: "Hold or tap" },
            ]}
            className="w-[330px]"
          />
        </SettingRow>
        <SettingRow
          icon={TextCursorInput}
          label="Edit with your voice"
          description={
            settings.cleanup.enabled
              ? "Select text in any app, hold this shortcut and say what to change, like “make this friendlier”. The result replaces your selection."
              : "Select text, hold this shortcut and say what to change. Needs AI cleanup, below."
          }
        >
          <CommandHotkeyPicker />
        </SettingRow>
      </Section>

      <Section title="Behavior">
        <SettingRow
          icon={Keyboard}
         
          label="Type while speaking"
          description="Insert short phrases as you speak. Escape stops further typing; words already inserted stay. Turn off to insert everything when you stop."
        >
          <Switch label="Type while speaking" checked={settings.liveTyping} onChange={(liveTyping) => save({ liveTyping })} />
        </SettingRow>
        {settings.liveTyping && settings.recordingMode !== "toggle" && settings.hotkey !== "Fn" && (
          <p className="py-3 pl-8 type-small text-ink-secondary">
            For live typing with this shortcut, choose Toggle mode. Other modes insert the transcript when you stop.
          </p>
        )}
        <SettingRow
          icon={CornerDownLeft}
          label="Finish and send"
          description={`Press ${isMac ? "Return" : "Enter"} while recording to stop, insert the text and send it, as in a chat or an AI prompt. Works when recording hands-free, not while you hold the hotkey.`}
        >
          <Switch label="Finish and send" checked={settings.finishAndSend} onChange={(finishAndSend) => save({ finishAndSend })} />
        </SettingRow>
        <SettingRow
          icon={Power}
          label="Launch at login"
          description={
            settings.launchAtLogin
              ? "Wordfall starts in the background when you log in, ready for your hotkey."
              : "Wordfall only runs when you open it."
          }
        >
          <Switch label="Launch at login" checked={settings.launchAtLogin} onChange={(launchAtLogin) => save({ launchAtLogin })} />
        </SettingRow>
        <SettingRow
          icon={Command}
         
          label={isMac ? "Show menu bar icon" : "Show tray icon"}
          description="Quick access to dictation and Wordfall's window."
        >
          <Switch checked={settings.showTrayIcon} onChange={(showTrayIcon) => save({ showTrayIcon })} />
        </SettingRow>
        <SettingRow
          icon={ClipboardCheck}
         
          label="Restore clipboard after pasting"
          description={
            settings.restoreClipboard
              ? "After Wordfall pastes, whatever you had copied is put back."
              : "The transcript stays on your clipboard after Wordfall pastes it."
          }
        >
          <Switch checked={settings.restoreClipboard} onChange={(restoreClipboard) => save({ restoreClipboard })} />
        </SettingRow>
        <SettingRow
          icon={AudioLines}
         
          label="Keep recordings"
          description={
            settings.saveRecordings
              ? "Each dictation's audio is kept on this computer, so you can replay it from History."
              : "Only the text is kept. Recordings you already have stay until you delete them in History."
          }
        >
          <Switch checked={settings.saveRecordings} onChange={(saveRecordings) => save({ saveRecordings })} />
        </SettingRow>
        <SettingRow
          icon={PanelBottom}
         
          label="Always show the recorder pill"
          description={
            settings.alwaysShowPill
              ? "The small recorder stays on screen even when you're not dictating."
              : "The recorder appears while you dictate and hides when you're done."
          }
        >
          <Switch checked={settings.alwaysShowPill} onChange={(alwaysShowPill) => save({ alwaysShowPill })} />
        </SettingRow>
        <SettingRow icon={PanelBottom} label="Recorder position" description="Where the pill appears on your screen.">
          <PillPositionPicker value={settings.pillPosition} onChange={(pillPosition) => save({ pillPosition })} />
        </SettingRow>
      </Section>

      <Section title="Transcript cleanup" description="Light, offline tidying after each dictation.">
        <SettingRow
          icon={Wand2}
         
          label="Remove filler words"
          description="Takes out “um”, “uh” and similar words. The meaning of what you said never changes."
        >
          <Switch checked={settings.autoEdit} onChange={(autoEdit) => save({ autoEdit })} />
        </SettingRow>
        <SettingRow
          icon={Sparkles}
         
          label="Smart trailing punctuation"
          description="When you dictate just an email, link, number or single word, the final period is dropped so it pastes clean. Sentences are left alone."
        >
          <Switch
            checked={settings.smartTrailingPunctuation}
            onChange={(smartTrailingPunctuation) => save({ smartTrailingPunctuation })}
          />
        </SettingRow>
        <SettingRow
          icon={Sparkles}
          label="Commas instead of colons and semicolons"
          description={
            settings.plainPunctuation
              ? "Colons and semicolons the speech model adds become commas, the way people usually type. Times like 3:30, links and the colon before a list stay."
              : "Punctuation stays as the speech model wrote it, colons and semicolons included."
          }
        >
          <Switch checked={settings.plainPunctuation} onChange={(plainPunctuation) => save({ plainPunctuation })} />
        </SettingRow>
        <SettingRow
          icon={Lightbulb}
         
          label="Learn from my corrections"
          description={
            settings.learnCorrections
              ? "When you fix a misheard name or term, or its capitals (github → GitHub), Wordfall adds it to Dictionary and gets it right next time. Learned words can be removed there."
              : "Corrections are saved but nothing is learned from them."
          }
        >
          <Switch
            checked={settings.learnCorrections}
            onChange={(learnCorrections) => save({ learnCorrections })}
          />
        </SettingRow>
        <SettingRow
          icon={BookA}
         
          label="Replacements and snippets"
          description="Word fixes and snippets (say “my calendar link”, get the link) live in Dictionary."
        />
      </Section>

      <SpokenSection />
      <CleanupSection />
      <AppToneSection />
      <LanguageSection />
      <SharingSection />
      <UpdatesSection />
      <ImportSection />
    </>
  );
}

function LanguageSection() {
  const { settings } = useStore();
  const save = useSave();

  const recents = settings.recentLanguages.filter((code) => code !== settings.language);
  const options: SelectOption<string>[] = [
    { value: "auto", label: "Detect automatically" },
    ...recents.map((code) => ({ value: code, label: languageName(code), detail: "Recent" })),
    ...LANGUAGES.filter(([code]) => !recents.includes(code)).map(([code, name]) => ({ value: code, label: name })),
  ];

  const choose = (language: string) => {
    const recentLanguages =
      language === "auto"
        ? settings.recentLanguages
        : [language, ...settings.recentLanguages.filter((c) => c !== language)].slice(0, 5);
    save({ language, recentLanguages });
  };

  return (
    <Section title="Spoken language">
      <SettingRow
        icon={Globe}
       
        label="Language you speak"
        description="A hint for transcription. It doesn't translate, and a wrong hint can give poor results, so detection is the safest choice."
      >
        <Select
          value={settings.language}
          options={options}
          onChange={choose}
          searchable
          aria-label="Spoken language"
          className="w-[240px]"
        />
      </SettingRow>
      <div className="py-3.5 pl-8 type-small text-ink-muted">
        For languages other than English, use a multilingual model. English-only models can only write English. With
        detection on, you can switch languages between sentences: Whisper listens again after each pause.
      </div>
    </Section>
  );
}

function UpdatesSection() {
  const { settings, status } = useStore();
  const save = useSave();
  const toast = useToast();
  const [checking, setChecking] = useState(false);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);

  const check = async () => {
    setChecking(true);
    try {
      const info = await api.checkForUpdate();
      if (info.available) setUpdate(info);
      else toast(`You're up to date. Wordfall ${info.currentVersion} is the latest version.`);
    } catch (error) {
      toast(errorMessage(error), "error");
    } finally {
      setChecking(false);
    }
  };

  return (
    <Section
      title="Updates"
      action={
        <Button size="sm" variant="ghost" icon={RotateCw} loading={checking} onClick={check}>
          {checking ? "Checking…" : "Check now"}
        </Button>
      }
    >
      <SettingRow
        icon={RefreshCw}
        label="Check for updates automatically"
        description={`Once a day, in the background. You have Wordfall ${status.version}.`}
      >
        <Switch checked={settings.autoUpdate} onChange={(autoUpdate) => save({ autoUpdate })} />
      </SettingRow>
      <UpdateDialog update={update} onClose={() => setUpdate(null)} />
    </Section>
  );
}

/** Only shown when SpeakType 1's data is on this computer. */
function ImportSection() {
  const toast = useToast();
  const [available, setAvailable] = useState<LegacyStatus["available"]>(null);
  const [importing, setImporting] = useState(false);

  useEffect(() => {
    api
      .getLegacyStatus()
      .then((status) => setAvailable(status.available))
      .catch(() => {});
  }, []);

  if (!available) return null;

  const run = async () => {
    setImporting(true);
    try {
      const result = await api.importLegacy();
      const what = describeImport(result);
      toast(what ? `Imported ${what} from SpeakType 1.` : "Everything from SpeakType 1 is already here.");
    } catch (error) {
      toast(errorMessage(error), "error");
    } finally {
      setImporting(false);
    }
  };

  return (
    <Section title="SpeakType 1">
      <SettingRow
        icon={Import}
       
        label="Import from SpeakType 1"
        description={`Found ${describeImport(available)} on this computer. Anything already here is skipped.`}
      >
        <Button loading={importing} onClick={run}>
          Import
        </Button>
      </SettingRow>
    </Section>
  );
}

function AudioTab() {
  const { settings } = useStore();
  const save = useSave();
  const [devices, setDevices] = useState<InputDevice[]>();
  const [refreshing, setRefreshing] = useState(false);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    setDevices(await api.listInputDevices());
    setRefreshing(false);
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const selectedExists = devices?.some((d) => d.id === settings.inputDevice);
  const rows = [
    { id: "", name: "System default", detail: devices?.find((d) => d.isDefault)?.name },
    ...(devices ?? []).map((d) => ({ id: d.id, name: d.name, detail: undefined })),
  ];


  return (
    <Section
      title="Microphone"
      description="Wordfall records from this input."
      action={
        <Button size="sm" variant="ghost" icon={RotateCw} loading={refreshing} onClick={refresh}>
          Refresh
        </Button>
      }
    >
      {devices && devices.length === 0 ? (
        <EmptyState icon={Mic} title="No microphones found" description="Connect a microphone, then refresh." />
      ) : (
        rows.map((row) => {
          const selected = row.id === settings.inputDevice || (row.id === "" && !selectedExists);
          return (
            <button
              key={row.id || "default"}
              type="button"
              role="radio"
              aria-checked={selected}
              onClick={() => save({ inputDevice: row.id })}
              className="group flex w-full items-center gap-3.5 py-4 text-left"
            >
              <span
                className={cn(
                  "flex size-[18px] shrink-0 items-center justify-center rounded-full border-[1.5px] transition-colors",
                  selected ? "border-accent" : "border-line-strong group-hover:border-ink-muted",
                )}
              >
                {selected && <span className="size-2 rounded-full bg-accent" />}
              </span>
              <span className="min-w-0 flex-1">
                <span className="block truncate type-label">{row.name}</span>
                {row.detail && <span className="block truncate type-caption text-ink-muted">{row.detail}</span>}
              </span>
              {selected && (
                <Badge tone="success" icon={Mic}>
                  In use
                </Badge>
              )}
            </button>
          );
        })
      )}
    </Section>
  );
}

function RecordingSection() {
  const { settings } = useStore();
  const save = useSave();
  return (
    <Section title="While you dictate">
      <SettingRow
        icon={Ear}
        label="Whisper mode"
        description={
          settings.boostQuietSpeech
            ? "Speaking softly? Quiet speech is turned up before it's transcribed. Silence and background noise are left alone."
            : "Audio is transcribed at the level it was recorded."
        }
      >
        <Switch label="Whisper mode" checked={settings.boostQuietSpeech} onChange={(boostQuietSpeech) => save({ boostQuietSpeech })} />
      </SettingRow>
      <SettingRow
        icon={Music}
        label="Pause media while dictating"
        description={
          settings.pauseMedia
            ? "Music and videos pause when you start recording, and carry on when you stop."
            : "Music and videos keep playing while you dictate."
        }
      >
        <Switch label="Pause media while dictating" checked={settings.pauseMedia} onChange={(pauseMedia) => save({ pauseMedia })} />
      </SettingRow>
    </Section>
  );
}

function PermissionsTab() {
  return (
    <Section title="App permissions" description="Wordfall needs both to dictate into other apps.">
      <PermissionList />
    </Section>
  );
}
