import { AudioLines, Mic, Square } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { AudioPlayer } from "@/components/AudioPlayer";
import { Button, Section, SettingRow, Spinner } from "@/components/ui";
import { api, errorMessage } from "@/lib/api";
import { formatClock } from "@/lib/format";
import { useNow } from "@/lib/hooks";
import { useStore } from "@/lib/store";
import { useTauriEvent } from "@/lib/useTauriEvent";

/** Long enough for a sentence or two; a forgotten test shouldn't keep the microphone open. */
const MAX_SECONDS = 15;

type Test =
  | { phase: "idle" }
  | { phase: "starting" }
  | { phase: "recording"; startedAt: number }
  | { phase: "stopping" }
  | { phase: "done"; audio: ArrayBuffer; transcript?: string; transcriptError?: string };

/** Records a short clip, plays it back as the model hears it, and shows what the model understood. */
export function MicTestSection() {
  const { settings } = useStore();
  const [test, setTest] = useState<Test>({ phase: "idle" });
  const [error, setError] = useState<string>();
  const [level, setLevel] = useState(0);
  const mounted = useRef(true);
  // The time limit and a click can both stop the same recording.
  const stopping = useRef(false);
  const recording = test.phase === "recording";
  const now = useNow(250, recording);
  const elapsed = recording ? (now - test.startedAt) / 1000 : 0;

  useTauriEvent<number>("mic-test-level", (e) => setLevel(e.payload));

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      api.cancelMicTest();
    };
  }, []);

  const start = async () => {
    setError(undefined);
    setLevel(0);
    setTest({ phase: "starting" });
    try {
      await api.startMicTest();
      if (!mounted.current) return api.cancelMicTest();
      setTest({ phase: "recording", startedAt: Date.now() });
    } catch (e) {
      setError(errorMessage(e));
      setTest({ phase: "idle" });
    }
  };

  const stop = async () => {
    if (stopping.current) return;
    stopping.current = true;
    setTest({ phase: "stopping" });
    let audio: ArrayBuffer;
    try {
      audio = await api.stopMicTest();
    } catch (e) {
      if (mounted.current) {
        setError(errorMessage(e));
        setTest({ phase: "idle" });
      }
      return;
    } finally {
      stopping.current = false;
    }
    if (!mounted.current) return;
    setTest({ phase: "done", audio });
    const result = await api.transcribeMicTest().then(
      (transcript) => ({ transcript }),
      (e) => ({ transcriptError: errorMessage(e) }),
    );
    if (mounted.current) setTest((t) => (t.phase === "done" && t.audio === audio ? { ...t, ...result } : t));
  };

  // Stops on its own, so a forgotten test doesn't keep the microphone open.
  useEffect(() => {
    if (recording && elapsed >= MAX_SECONDS) stop();
  }, [recording, elapsed]);

  const busy = test.phase === "starting" || test.phase === "stopping";
  return (
    <Section title="Test your microphone">
      <SettingRow
        icon={AudioLines}
        label={recording ? `Recording… ${formatClock(elapsed)}` : "Hear yourself"}
        description={
          recording
            ? "Say a sentence the way you'd dictate it, then stop."
            : `Record a few seconds, then play it back as the speech model hears it${
                settings.boostQuietSpeech ? ", with Whisper mode" : ""
              }. Listen for background noise and check what it understood.`
        }
      >
        {recording ? (
          <Button size="sm" variant="danger" icon={Square} onClick={stop}>
            Stop
          </Button>
        ) : (
          <Button size="sm" icon={Mic} loading={busy} disabled={busy} onClick={start}>
            {test.phase === "done" ? "Record again" : "Record"}
          </Button>
        )}
      </SettingRow>

      {recording && (
        <div className="pb-4">
          <div
            role="meter"
            aria-label="Microphone level"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(level * 100)}
            className="h-1.5 overflow-hidden rounded-full bg-hover"
          >
            <div className="h-full rounded-full bg-accent transition-[width] duration-75" style={{ width: `${level * 100}%` }} />
          </div>
        </div>
      )}

      {error && <p className="pb-4 type-small text-danger">{error}</p>}

      {test.phase === "done" && (
        <div className="space-y-3 pb-4">
          <AudioPlayer audio={test.audio} />
          <div className="rounded-card border border-line-subtle px-3 py-2.5 type-small">
            {test.transcript !== undefined ? (
              <p className="text-ink">{test.transcript}</p>
            ) : test.transcriptError ? (
              <p className="text-ink-muted">{test.transcriptError}</p>
            ) : (
              <p className="flex items-center gap-2 text-ink-muted">
                <Spinner size={12} /> Transcribing…
              </p>
            )}
          </div>
        </div>
      )}
    </Section>
  );
}
