import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import {
  ArrowUpRight,
  Check,
  ChevronsUpDown,
  Cpu,
  Globe,
  Hand,
  Mic,
  Power,
  Settings,
  Square,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { LogoMark } from "@/components/brand/LogoMark";
import { IconButton, Spinner } from "@/components/ui";
import { api, type HistoryItem, type InputDevice } from "@/lib/api";
import { cn } from "@/lib/cn";
import {
  TYPING_WORDS_PER_MINUTE,
  formatClock,
  formatNumber,
  formatRelative,
  hotkeyParts,
  startOfDay,
} from "@/lib/format";
import { useCopy, useDictationState, useEngineStatus, useHistory, useNow } from "@/lib/hooks";
import { languageName } from "@/lib/languages";
import { showLanguageMenu, showMicrophoneMenu, showModeMenu, showModelMenu } from "@/lib/menus";
import { useStore } from "@/lib/store";
import { useApplyTheme } from "@/lib/theme";

const PANEL_WIDTH = 356;

/** The panel that opens from the menu bar icon: dictate, glance at today, switch quickly, grab a recent transcript. */
export function TrayPanel() {
  const { settings } = useStore();
  const container = useRef<HTMLDivElement>(null);
  useApplyTheme(settings.theme);

  // Size the window to the content so there's no empty transparent area.
  useEffect(() => {
    const element = container.current;
    if (!element) return;
    const observer = new ResizeObserver(() => {
      getCurrentWindow().setSize(new LogicalSize(PANEL_WIDTH, Math.ceil(element.offsetHeight)));
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && api.hideTrayPanel();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div ref={container} className="p-2" style={{ width: PANEL_WIDTH }}>
      <div className="overflow-hidden rounded-panel border border-line bg-surface shadow-elevated">
        <Header />
        <Today />
        <QuickSettings />
        <Recent />
        <Footer />
      </div>
    </div>
  );
}

function Header() {
  const { settings, models } = useStore();
  const dictation = useDictationState();
  const engine = useEngineStatus();
  const recording = dictation.phase === "recording";
  const transcribing = dictation.phase === "transcribing";
  const now = useNow(500, recording);
  const model = models.find((m) => m.id === settings.selectedModel && m.downloaded);

  let status: { dot: string; label: string };
  if (recording) status = { dot: "bg-recording animate-pulse", label: `Recording ${formatClock((now - dictation.startedAtMs) / 1000)}` };
  else if (transcribing) status = { dot: "bg-ink-muted", label: "Transcribing…" };
  else if (!model) status = { dot: "bg-warning", label: "No model" };
  else if (engine.loading === model.id) status = { dot: "bg-ink-muted", label: "Loading model…" };
  else status = { dot: "bg-success", label: "Ready" };

  const hotkey = hotkeyParts(settings.hotkey).join(" ");
  const verb = settings.recordingMode === "toggle" ? "press" : "hold";

  return (
    <div className="px-4 pt-4 pb-4 [background-image:var(--app-wash)]">
      <div className="flex items-center gap-2.5">
        <LogoMark className="size-7" />
        <div className="min-w-0 flex-1">
          <div className="type-label">Wordfall</div>
          <div className="truncate type-caption text-ink-muted">{model ? model.name : "Download a model to start"}</div>
        </div>
        <span className="flex h-6 items-center gap-1.5 rounded-full border border-line bg-surface/70 px-2.5 type-caption font-medium text-ink-secondary tabular-nums">
          <span className={cn("size-1.5 rounded-full", status.dot)} />
          {status.label}
        </span>
      </div>

      {!model ? (
        <button
          type="button"
          onClick={() => api.openMainWindow("models")}
          className="mt-4 flex h-11 w-full items-center justify-center gap-2 rounded-card bg-accent-flow type-label text-on-accent transition-[filter] hover:brightness-110"
        >
          Choose a model <ArrowUpRight size={16} />
        </button>
      ) : recording ? (
        <button
          type="button"
          onClick={() => api.toggleDictation()}
          className="mt-4 flex h-11 w-full items-center justify-center gap-2 rounded-card bg-primary type-label text-on-primary transition-colors hover:bg-primary-hover"
        >
          <Square size={14} fill="currentColor" className="text-recording" /> Stop and paste
        </button>
      ) : transcribing ? (
        <div className="mt-4 flex h-11 w-full items-center justify-center gap-2 rounded-card bg-hover type-label text-ink-secondary">
          <Spinner size={14} /> Transcribing…
        </div>
      ) : (
        <button
          type="button"
          onClick={() => api.toggleDictation()}
          className="mt-4 flex h-11 w-full items-center justify-between rounded-card bg-accent-flow px-4 type-label text-on-accent transition-[filter] hover:brightness-110"
        >
          <span className="flex items-center gap-2">
            <Mic size={16} /> Start dictation
          </span>
          <span className="type-caption text-on-accent/80">
            or {verb} {hotkey}
          </span>
        </button>
      )}
    </div>
  );
}

function Today() {
  const { stats } = useHistory();
  const now = useNow(60_000);
  const today = stats.filter((s) => s.createdAt >= startOfDay(now));
  const words = today.reduce((sum, s) => sum + s.wordCount, 0);
  const minutesSaved = Math.round(words / TYPING_WORDS_PER_MINUTE);

  const saved = minutesSaved < 60 ? `${minutesSaved} min` : `${Math.floor(minutesSaved / 60)}h ${minutesSaved % 60}m`;
  // One quiet line, like the dashboard's.
  return (
    <p className="flex justify-center gap-4 border-y border-line-subtle px-4 py-2.5 type-caption text-ink-muted">
      <span>
        <b className="font-bold text-ink tabular-nums">{formatNumber(words)}</b> words today
      </span>
      <span>
        <b className="font-bold text-ink tabular-nums">{formatNumber(today.length)}</b>{" "}
        {today.length === 1 ? "dictation" : "dictations"}
      </span>
      <span>
        <b className="font-bold text-ink tabular-nums">{saved}</b> saved
      </span>
    </p>
  );
}

function QuickSettings() {
  const { settings, models } = useStore();
  const [devices, setDevices] = useState<InputDevice[]>([]);
  useEffect(() => {
    api.listInputDevices().then(setDevices);
  }, [settings.inputDevice]);

  const model = models.find((m) => m.id === settings.selectedModel);
  const device = devices.find((d) => d.id === settings.inputDevice);

  const rows: { icon: LucideIcon; label: string; value: string; onClick: () => void }[] = [
    { icon: Cpu, label: "Model", value: model?.name ?? "None", onClick: () => showModelMenu(settings) },
    { icon: Globe, label: "Language", value: settings.language === "auto" ? "Auto-detect" : languageName(settings.language), onClick: () => showLanguageMenu(settings) },
    { icon: Mic, label: "Microphone", value: device?.name ?? "System default", onClick: () => showMicrophoneMenu(settings) },
    {
      icon: Hand,
      label: "Mode",
      value: { hold: "Hold to talk", toggle: "Toggle", hybrid: "Hold or tap" }[settings.recordingMode],
      onClick: () => showModeMenu(settings),
    },
  ];

  return (
    <div className="border-b border-line-subtle py-1.5">
      {rows.map(({ icon: Icon, label, value, onClick }) => (
        <button
          key={label}
          type="button"
          onClick={onClick}
          className="flex h-9 w-full items-center gap-3 px-4 text-left transition-colors hover:bg-hover"
        >
          <Icon size={15} className="shrink-0 text-ink-muted" />
          <span className="w-24 shrink-0 type-small text-ink-secondary">{label}</span>
          <span className="min-w-0 flex-1 truncate text-right type-small font-medium">{value}</span>
          <ChevronsUpDown size={13} className="shrink-0 text-ink-muted" />
        </button>
      ))}
    </div>
  );
}

function Recent() {
  const { items } = useHistory();
  const now = useNow(60_000);
  const { copy, copiedKey } = useCopy();
  const recent = (items ?? []).slice(0, 3);

  return (
    <div className="border-b border-line-subtle pb-1.5">
      <div className="flex items-center px-4 pt-3 pb-1">
        <span className="flex-1 type-overline text-ink-muted">Recently said</span>
        {recent.length > 0 && (
          <button
            type="button"
            onClick={() => api.openMainWindow("history")}
            className="type-caption font-medium text-ink-secondary transition-colors hover:text-ink"
          >
            View all
          </button>
        )}
      </div>
      {recent.length === 0 ? (
        <p className="px-4 pt-1 pb-3 type-small text-ink-muted">Your latest dictations will show up here.</p>
      ) : (
        recent.map((item) => (
          <RecentItem
            key={item.id}
            item={item}
            now={now}
            copied={copiedKey === item.id}
            onCopy={() => copy(item.transcript, item.id)}
          />
        ))
      )}
    </div>
  );
}

function RecentItem({
  item,
  now,
  copied,
  onCopy,
}: {
  item: HistoryItem;
  now: number;
  copied: boolean;
  onCopy: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onCopy}
      title="Copy"
      className="group flex w-full items-start gap-3 px-4 py-2 text-left transition-colors hover:bg-hover"
    >
      <span className="min-w-0 flex-1">
        <span className="line-clamp-2 type-small text-ink">{item.transcript}</span>
        <span className="type-caption text-ink-muted">{formatRelative(item.createdAt, now)}</span>
      </span>
      <span
        className={cn(
          "mt-0.5 flex h-5 shrink-0 items-center gap-1 type-caption font-medium transition-opacity",
          copied ? "text-success" : "text-ink-secondary opacity-0 group-hover:opacity-100",
        )}
      >
        {copied ? (
          <>
            <Check size={12} /> Copied
          </>
        ) : (
          "Copy"
        )}
      </span>
    </button>
  );
}

function Footer() {
  return (
    <div className="flex items-center gap-1 px-2 py-2">
      <button
        type="button"
        onClick={() => api.openMainWindow()}
        className="flex h-8 items-center gap-1.5 rounded-control px-2.5 type-small font-medium text-ink-secondary transition-colors hover:bg-hover hover:text-ink"
      >
        Open Wordfall <ArrowUpRight size={14} />
      </button>
      <span className="flex-1" />
      <IconButton icon={Settings} label="Settings" size="sm" onClick={() => api.openMainWindow("settings")} />
      <IconButton icon={Power} label="Quit Wordfall" size="sm" onClick={() => api.quitApp()} />
    </div>
  );
}
