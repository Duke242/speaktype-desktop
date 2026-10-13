import { ChevronsUpDown, Globe, Hand, Lock, Mic, Repeat, type LucideIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { api, type PillState, type Settings } from "@/lib/api";
import { cn } from "@/lib/cn";
import { formatClock } from "@/lib/format";
import { useNow } from "@/lib/hooks";
import { useApplyTheme } from "@/lib/theme";
import { useTauriEvent } from "@/lib/useTauriEvent";
import { showLanguageMenu, showMicrophoneMenu, showModeMenu, showModelMenu } from "@/lib/menus";
import { Waveform } from "./Waveform";

// Pill sizes per phase, from the macOS recorder.
const SIZES = {
  idle: { width: 58, height: 24 },
  warming: { width: 320, height: 44 },
  processing: { width: 220, height: 44 },
  recording: { width: 250, height: 44 },
  recordingExpanded: { width: 460, height: 44 },
};

export function Pill({ preview = false }: { preview?: boolean }) {
  // Add ?locked=1 to preview a hands-free recording.
  const [state, setState] = useState<PillState>(() =>
    preview
      ? { phase: "recording", startedAtMs: Date.now(), locked: new URLSearchParams(location.search).has("locked") }
      : { phase: "idle" },
  );
  const [settings, setSettings] = useState<Settings>();
  const [hovered, setHovered] = useState(false);
  const level = useRef({ value: 0, updatedAt: 0 });
  const [deviceName, setDeviceName] = useState("Default");
  const warmingSince = useRef(0);
  const now = useNow(state.phase === "warming" ? 1000 : 500, state.phase === "recording" || state.phase === "warming");
  useApplyTheme(settings?.theme ?? "system");

  useEffect(() => {
    if (!preview) return;
    // A deterministic speech envelope for inspecting motion without a microphone.
    const timer = window.setInterval(() => {
      const now = performance.now();
      const t = now / 1000;
      const speaking = t % 6 < 4.6;
      level.current = { value: speaking ? 0.15 + 0.65 * Math.abs(Math.sin(t * 4.2)) * (0.6 + 0.4 * Math.sin(t * 1.7) ** 2) : 0, updatedAt: now };
    }, 33);
    return () => window.clearInterval(timer);
  }, [preview]);

  const refreshSettings = async () => {
    const next = await api.getSettings();
    setSettings(next);
    const devices = await api.listInputDevices();
    const device = devices.find((d) => d.id === next.inputDevice);
    setDeviceName(device ? device.name.split(" ")[0] : "Default");
  };

  useEffect(() => {
    refreshSettings();
  }, []);
  useTauriEvent("settings-changed", () => refreshSettings());
  useTauriEvent<PillState>("pill-state", ({ payload }) => {
    if (payload.phase === "warming" && state.phase !== "warming") warmingSince.current = Date.now();
    if (payload.phase === "recording" && state.phase !== "recording") level.current = { value: 0, updatedAt: 0 };
    if (payload.phase !== "recording") setHovered(false);
    setState(payload);
  });
  useTauriEvent<number>("pill-level", ({ payload }) => {
    level.current = { value: payload, updatedAt: performance.now() };
  });

  const expanded = state.phase === "recording" && hovered;
  const size = expanded ? SIZES.recordingExpanded : SIZES[state.phase];
  const active = state.phase !== "idle";

  return (
    <div className="flex h-full items-center justify-center">
      <div
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
        onContextMenu={(event) => {
          // Right-click to switch model.
          event.preventDefault();
          if (settings) showModelMenu(settings);
        }}
        style={{ width: size.width, height: size.height, borderRadius: size.height / 2 }}
        className={cn(
          "relative flex items-center overflow-hidden text-white",
          "transition-[width,height,border-radius,box-shadow] duration-[450ms] ease-out-soft",
          active
            ? "bg-pill bg-linear-to-b from-white/[0.06] to-transparent shadow-elevated ring-1 ring-pill-wave/15"
            : "bg-pill/85 shadow-elevated ring-1 ring-pill-wave/15 backdrop-blur-xl",
        )}
      >
        {state.phase === "idle" && <IdleBars />}
        {state.phase === "warming" && <Warming seconds={Math.floor((now - warmingSince.current) / 1000)} />}
        {state.phase === "processing" && (
          <p key={state.message} className="w-full animate-fade-in truncate px-4 text-center type-small font-medium">
            {state.message}
          </p>
        )}
        {state.phase === "recording" && (
          <div className="flex w-full animate-fade-in items-center gap-2.5 px-3.5">
            <RecordingDot />
            {state.locked && <LockBadge />}
            <Waveform level={level} />
            <span className="shrink-0 type-caption font-medium text-white/60 tabular-nums">
              {formatClock((now - state.startedAtMs) / 1000)}
            </span>
            {expanded && settings && (
              <div className="flex shrink-0 animate-pop-in items-center gap-1.5 pl-1">
                <Chip icon={Mic} label={deviceName} onClick={() => showMicrophoneMenu(settings)} />
                <Chip
                  icon={settings.recordingMode === "toggle" ? Repeat : Hand}
                  label={{ hold: "Hold", toggle: "Toggle", hybrid: "Hybrid" }[settings.recordingMode]}
                  onClick={() => showModeMenu(settings)}
                />
                <Chip
                  icon={Globe}
                  label={settings.language === "auto" ? "Auto" : settings.language.toUpperCase()}
                  onClick={() => showLanguageMenu(settings)}
                />
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function IdleBars() {
  return (
    <div className="flex w-full items-center justify-center gap-[3px]">
      {[0.5, 0.85, 1, 0.85, 0.5].map((scale, i) => (
        <span
          key={i}
          className="w-[2.5px] rounded-full bg-linear-to-b from-pill-wave to-pill-wave-deep"
          style={{ height: 12 * scale }}
        />
      ))}
    </div>
  );
}

function Warming({ seconds }: { seconds: number }) {
  return (
    <div className="flex w-full animate-fade-in items-center gap-2.5 px-4">
      <span
        className="size-3.5 shrink-0 animate-spin-fast rounded-full"
        style={{
          background: "conic-gradient(from 0deg, transparent, var(--color-pill-wave) 295deg, transparent 296deg)",
          mask: "radial-gradient(farthest-side, transparent calc(100% - 2px), #000 calc(100% - 1.5px))",
          WebkitMask: "radial-gradient(farthest-side, transparent calc(100% - 2px), #000 calc(100% - 1.5px))",
        }}
      />
      <div className="min-w-0 flex-1 leading-tight">
        <p className="truncate type-small font-medium text-white/90">Getting model ready…</p>
        {seconds >= 3 && (
          <p className="truncate type-caption text-white/55">
            {seconds}s · {seconds >= 25 ? "almost there, first load only" : "first load only"}
          </p>
        )}
      </div>
      <span className="type-caption font-medium text-white/40">esc</span>
    </div>
  );
}

function RecordingDot() {
  return (
    <button
      type="button"
      title="Recording. Click or press your hotkey to stop."
      onClick={() => api.toggleDictation()}
      className="relative flex size-6 shrink-0 items-center justify-center"
    >
      <span className="absolute size-2.5 animate-ping rounded-full bg-recording/35 [animation-duration:1.4s]" />
      <span className="relative size-2.5 rounded-full bg-recording" />
    </button>
  );
}

function LockBadge() {
  return (
    <span
      title="Hands-free. Press your hotkey to stop."
      className="flex size-5 shrink-0 animate-pop-in items-center justify-center rounded-full bg-white/12 text-white/80"
    >
      <Lock size={10} strokeWidth={2.75} />
    </span>
  );
}

function Chip({ icon: Icon, label, onClick }: { icon: LucideIcon; label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex h-6 max-w-[96px] items-center gap-1 rounded-full bg-white/12 px-2 type-caption font-medium text-white/90 transition-colors hover:bg-white/20"
    >
      <Icon size={11} strokeWidth={2.5} className="shrink-0" />
      <span className="truncate">{label}</span>
      <ChevronsUpDown size={10} className="shrink-0 text-white/45" />
    </button>
  );
}
