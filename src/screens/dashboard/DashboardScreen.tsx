import { ArrowRight, Check, Copy, Pause, Play, Sparkles, TriangleAlert } from "lucide-react";
import { Fragment, useCallback, useEffect, useRef, useState } from "react";
import type { Route } from "@/app/routes";
import { HeroWaves, KeyRipple } from "@/components/brand/KeyRipple";
import { Button, Callout, IconButton, Spinner, Switch, useToast } from "@/components/ui";
import { api, errorMessage, type DictionaryEntry, type HistoryItem, type StatsEntry } from "@/lib/api";
import { cn } from "@/lib/cn";
import { formatMinutesSaved, formatNumber, hotkeyParts, plural } from "@/lib/format";
import { useCopy, useEngineStatus, useHistory, useNow } from "@/lib/hooks";
import { hotkeyBlockedMessage, useWhileHotkeyBlocked } from "@/lib/hotkeyBlocked";
import { useStore } from "@/lib/store";
import { showSettingsTab } from "@/screens/settings/SettingsScreen";
import { useTauriEvent } from "@/lib/useTauriEvent";
import { feedTime, hotkeyVerb, recentlyLearned, summarize, thisDevice } from "./summary";

/**
 * Home: an invitation to talk. The hotkey sits in a pool of ripples with one
 * line of reassurance and one line of numbers under it, and what was said
 * recently flows beneath, fading out toward History.
 */
export function DashboardScreen({ onNavigate }: { onNavigate: (route: Route) => void }) {
  const { items, stats } = useHistory();
  const now = useNow(60_000);
  // Until there's something to show, the invitation is the whole screen.
  const empty = items !== undefined && items.length === 0;

  return (
    <div className="relative flex h-full flex-col overflow-x-hidden overflow-y-auto no-scrollbar">
      <LiveTypingToggle />
      <Hero stats={stats} now={now} centered={empty} onNavigate={onNavigate} />
      {items && items.length > 0 && <RecentlySaid items={items} now={now} onNavigate={onNavigate} />}
    </div>
  );
}

/** The one setting worth having at hand: type phrases as you speak, or everything at the end. */
function LiveTypingToggle() {
  const { settings, updateSettings } = useStore();
  const toast = useToast();
  return (
    <div
      title="Insert short phrases as you speak, instead of everything when you stop."
      className="absolute top-4 right-6 z-20 flex h-9 items-center gap-3 rounded-full border border-line bg-surface/70 pr-2 pl-3.5 type-small font-medium text-ink-secondary"
    >
      <span aria-hidden>Type while speaking</span>
      <Switch
        label="Type while speaking"
        checked={settings.liveTyping}
        onChange={(liveTyping) => updateSettings({ liveTyping }).catch((e) => toast(errorMessage(e), "error"))}
      />
    </div>
  );
}

function Hero({
  stats,
  now,
  centered,
  onNavigate,
}: {
  stats: StatsEntry[];
  now: number;
  centered: boolean;
  onNavigate: (route: Route) => void;
}) {
  const { settings, status, models, refreshStatus } = useStore();
  const model = models.find((m) => m.id === settings.selectedModel && m.downloaded);
  const keys = hotkeyParts(settings.hotkey).join(" + ");
  useWhileHotkeyBlocked(status.hotkeyBlocked, refreshStatus);

  // Anything stopping dictation from working, with what fixes it.
  const notices: { text: string; action?: "settings" | "accessibility" }[] = [
    ...(status.hotkeyError ? [{ text: `${status.hotkeyError}. Choose another hotkey in Settings.`, action: "settings" as const }] : []),
    ...(status.hotkeyBlocked ? [{ text: hotkeyBlockedMessage(keys), action: "accessibility" as const }] : []),
    ...status.setupNotes.map((text) => ({ text })),
  ];

  return (
    <section
      className={cn(
        "relative flex shrink-0 flex-col items-center px-10 text-center",
        centered ? "my-auto py-16" : "pt-[124px]",
      )}
    >
      <div className="relative flex w-full justify-center">
        {/* A pool of light, its brightest point behind the keys. */}
        <div
          aria-hidden
          className="pointer-events-none absolute top-1/2 left-1/2 h-[800px] w-[1200px] -translate-1/2 bg-hero-wash"
        />
        <HeroWaves className="absolute -inset-x-10 top-1/2 h-[120px] -translate-y-1/2" />
        <KeyRipple hotkey={settings.hotkey} />
      </div>

      <h1 className="mt-[42px] type-display text-ink">
        {hotkeyVerb(settings.recordingMode)} <span className="text-accent-ink">{keys}</span> and talk.
      </h1>
      <p className="mt-2 type-feed text-ink-secondary">
        Your words are typed wherever your cursor is. Nothing leaves {thisDevice(status.os)}.
      </p>

      {model ? (
        <StatusLine
          modelId={model.id}
          modelName={model.name}
          stats={stats}
          now={now}
          onNavigate={onNavigate}
        />
      ) : (
        <div className="mt-6 w-full max-w-[560px] text-left">
          <Callout
            icon={Sparkles}
            tone="accent"
            title="Download a model to start dictating"
            action={
              <Button variant="accent" size="sm" icon={ArrowRight} onClick={() => onNavigate("models")}>
                Choose a model
              </Button>
            }
          >
            Models run entirely on {thisDevice(status.os)}. Your voice never leaves it.
          </Callout>
        </div>
      )}

      {notices.length > 0 && (
        <div className="mt-4 flex w-full max-w-[560px] flex-col gap-2 text-left">
          {notices.map((notice) => (
            <Callout
              key={notice.text}
              icon={TriangleAlert}
              action={
                notice.action === "settings" ? (
                  <Button size="sm" variant="ghost" onClick={() => onNavigate("settings")}>
                    Open Settings
                  </Button>
                ) : notice.action === "accessibility" ? (
                  <Button size="sm" variant="ghost" onClick={() => api.openPermissionSettings("accessibility")}>
                    Open Accessibility
                  </Button>
                ) : undefined
              }
            >
              {notice.text}
            </Callout>
          ))}
        </div>
      )}
    </section>
  );
}

/** Ready status, model and AI cleanup, then today's count, total words and time saved, in one quiet line. */
function StatusLine({
  modelId,
  modelName,
  stats,
  now,
  onNavigate,
}: {
  modelId: string;
  modelName: string;
  stats: StatsEntry[];
  now: number;
  onNavigate: (route: Route) => void;
}) {
  const engine = useEngineStatus();
  const ready = engine.loaded === modelId;
  const summary = summarize(stats, now);

  return (
    <div className="mt-7 flex flex-wrap items-center justify-center gap-x-5 gap-y-2 rounded-panel border border-line bg-surface/70 px-5 py-2 type-small font-medium text-ink-muted shadow-card">
      <span className="flex items-center gap-2">
        {engine.loading === modelId ? (
          <>
            <Spinner size={12} className="text-accent-ink" />
            Getting {modelName} ready…
          </>
        ) : (
          <>
            <span className="relative flex size-2">
              {/* A slow pulse while the model is loaded and listening for the hotkey. */}
              {ready && (
                <span className="absolute inset-0 rounded-full bg-success opacity-60 motion-safe:animate-ping motion-safe:[animation-duration:2s]" />
              )}
              <span
                className={cn(
                  "relative size-2 rounded-full",
                  ready
                    ? "bg-success shadow-[0_0_0_4px_var(--color-success-soft)]"
                    : "bg-ink-disabled",
                )}
              />
            </span>
            {ready ? `Ready · ${modelName}` : modelName}
          </>
        )}
      </span>
      <CleanupStatus onNavigate={onNavigate} />
      {stats.length > 0 && (
        <>
          <span>
            <Figure>{formatNumber(summary.today)}</Figure> today
          </span>
          <span>
            <Figure>{formatNumber(summary.totalWords)}</Figure> {summary.totalWords === 1 ? "word" : "words"}
            {summary.pages > 0 && ` · about ${plural(summary.pages, "page")} you didn't type`}
          </span>
          <span>
            <Figure>{formatMinutesSaved(summary.minutesSaved)}</Figure> saved
          </span>
        </>
      )}
    </div>
  );
}

/** Whether dictations are tidied by a model or pasted as heard, so turning cleanup off is never a surprise. */
function CleanupStatus({ onNavigate }: { onNavigate: (route: Route) => void }) {
  const { settings } = useStore();
  const [setUp, setSetUp] = useState<boolean>();
  const refresh = useCallback(() => {
    api
      .getCleanupStatus()
      .then((status) => setSetUp(status.setUp))
      .catch(() => {});
  }, []);
  useEffect(refresh, [refresh]);
  // Choosing or downloading a model changes whether cleanup can run.
  useTauriEvent("settings-changed", refresh);
  useTauriEvent("models-changed", refresh);

  // Switched on but with no model ready, transcripts are pasted as heard, the same as off.
  const state = !settings.cleanup.enabled ? "off" : setUp === false ? "unset" : "on";
  const { label, hint } = {
    on: { label: "AI cleanup on", hint: "Transcripts are tidied by your AI cleanup model." },
    off: { label: "AI cleanup off", hint: "Transcripts are pasted as heard. Turn on AI cleanup in Settings." },
    unset: { label: "AI cleanup not set up", hint: "Transcripts are pasted as heard until you choose a model in Settings." },
  }[state];

  return (
    <button
      type="button"
      onClick={() => {
        showSettingsTab("ai");
        onNavigate("settings");
      }}
      title={hint}
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full hover:text-ink-secondary hover:underline",
        state === "on" && "text-accent-ink",
      )}
    >
      {state === "unset" ? <TriangleAlert size={12} strokeWidth={2.25} /> : <Sparkles size={12} strokeWidth={2.25} />}
      {label}
    </button>
  );
}

function Figure({ children }: { children: string }) {
  return <b className="font-bold text-ink tabular-nums">{children}</b>;
}

/** Plays one history recording at a time. */
function usePlayback() {
  const current = useRef<{ id: string; audio: HTMLAudioElement; url: string } | null>(null);
  const request = useRef(0);
  const [playingId, setPlayingId] = useState<string | null>(null);

  const stop = useCallback(() => {
    request.current++;
    if (current.current) {
      current.current.audio.pause();
      URL.revokeObjectURL(current.current.url);
      current.current = null;
    }
    setPlayingId(null);
  }, []);

  const toggle = useCallback(
    async (id: string) => {
      const wasPlaying = current.current?.id === id;
      stop();
      if (wasPlaying) return;
      const token = request.current;
      try {
        const buffer = await api.readHistoryAudio(id);
        if (token !== request.current) return;
        const url = URL.createObjectURL(new Blob([buffer], { type: "audio/wav" }));
        const audio = new Audio(url);
        audio.onended = stop;
        current.current = { id, audio, url };
        setPlayingId(id);
        await audio.play();
      } catch {
        if (token === request.current) stop();
      }
    },
    [stop],
  );

  useEffect(() => stop, [stop]);
  return { playingId, toggle };
}

/** Enough rows to run past the bottom of the window, where the feed fades out. */
const RECENT_COUNT = 8;

function RecentlySaid({
  items,
  now,
  onNavigate,
}: {
  items: HistoryItem[];
  now: number;
  onNavigate: (route: Route) => void;
}) {
  const { settings } = useStore();
  const { copy, copiedKey } = useCopy();
  const playback = usePlayback();
  const learned = recentlyLearned(settings.dictionary);

  return (
    <section className="relative mx-auto mt-[60px] flex min-h-[240px] w-full max-w-[800px] flex-1 flex-col px-10">
      <div className="flex items-center justify-between border-b border-line pb-3.5">
        <h2 className="type-overline text-ink-muted">Recently said</h2>
        <button
          type="button"
          onClick={() => onNavigate("history")}
          className="inline-flex items-center gap-1 type-small font-medium text-accent-ink hover:underline"
        >
          All history <ArrowRight size={14} />
        </button>
      </div>

      <ol className="min-h-0 flex-1 overflow-hidden fade-out-bottom">
        {items.slice(0, RECENT_COUNT).map((item, i) => (
          <Fragment key={item.id}>
            <FeedRow
              item={item}
              now={now}
              copied={copiedKey === item.id}
              playing={playback.playingId === item.id}
              onCopy={() => copy(item.transcript, item.id)}
              onPlay={() => playback.toggle(item.id)}
            />
            {/* Learned words show up as a moment of their own, just under the newest transcript. */}
            {i === 0 && learned.length > 0 && <LearnedRow entries={learned} onOpen={() => onNavigate("dictionary")} />}
          </Fragment>
        ))}
      </ol>
    </section>
  );
}

const ROW = "grid grid-cols-[72px_minmax(0,1fr)_88px] items-baseline gap-5 border-b border-line py-4";

function FeedRow({
  item,
  now,
  copied,
  playing,
  onCopy,
  onPlay,
}: {
  item: HistoryItem;
  now: number;
  copied: boolean;
  playing: boolean;
  onCopy: () => void;
  onPlay: () => void;
}) {
  return (
    <li className="group relative">
      <button type="button" onClick={onCopy} title="Copy" className={cn(ROW, "w-full text-left")}>
        <time className="type-small text-ink-muted tabular-nums">{feedTime(item.createdAt, now)}</time>
        <p className="line-clamp-2 type-feed text-ink">{item.transcript}</p>
        {/* The word count turns into the copy hint on hover. */}
        <span className="grid justify-items-end type-caption">
          <span
            className={cn(
              "col-start-1 row-start-1 text-ink-muted tabular-nums transition-opacity group-hover:opacity-0",
              (copied || playing) && "opacity-0",
            )}
          >
            {plural(item.wordCount, "word")}
          </span>
          <span
            className={cn(
              "col-start-1 row-start-1 inline-flex items-center gap-1 font-medium whitespace-nowrap transition-opacity",
              copied ? "text-success" : "text-accent-ink opacity-0 group-hover:opacity-100",
              playing && !copied && "invisible",
            )}
          >
            {copied ? <Check size={12} strokeWidth={2.5} /> : <Copy size={12} strokeWidth={2.25} />}
            {copied ? "Copied" : "Copy"}
          </span>
        </span>
      </button>
      {item.audioPath && (
        <IconButton
          icon={playing ? Pause : Play}
          label={playing ? "Stop" : "Play recording"}
          size="sm"
          onClick={onPlay}
          className={cn(
            "absolute top-3 right-[60px] transition-[opacity,background-color,color]",
            playing ? "bg-hover text-ink" : "opacity-0 group-hover:opacity-100 focus-visible:opacity-100",
          )}
        />
      )}
    </li>
  );
}

function LearnedRow({ entries, onOpen }: { entries: DictionaryEntry[]; onOpen: () => void }) {
  const one = entries.length === 1;
  return (
    <li>
      <button type="button" onClick={onOpen} title="Open Dictionary" className={cn(ROW, "w-full items-center py-3.5 text-left")}>
        <Sparkles size={16} strokeWidth={1.75} className="text-accent-ink" />
        <p className="type-body text-ink-secondary">
          Learned{" "}
          {entries.map((entry) => (
            <span key={entry.id} className="mx-0.5 rounded-inner bg-accent-soft px-2 py-0.5 font-medium text-accent-ink">
              {entry.replacement}
            </span>
          ))}{" "}
          from your {one ? "correction" : "corrections"}. {one ? "It" : "They"}'ll be spelled right from now on.
        </p>
        <span />
      </button>
    </li>
  );
}
