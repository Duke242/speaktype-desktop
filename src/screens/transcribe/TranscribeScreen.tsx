import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { Check, CircleAlert, Copy, FileAudio, Mic, Square, Upload } from "lucide-react";
import { useEffect, useState } from "react";
import { Button, Callout, Card, IconTile, Page, PageHeader, Spinner } from "@/components/ui";
import { api, errorMessage, type DictationResult } from "@/lib/api";
import { cn } from "@/lib/cn";
import { formatClock, formatDuration } from "@/lib/format";
import { useCopy, useDictationState, useNow } from "@/lib/hooks";
import { useTauriEvent } from "@/lib/useTauriEvent";

const EXTENSIONS = ["mp3", "m4a", "aac", "wav", "flac", "ogg", "oga", "opus", "aiff", "aif", "caf", "mp4", "mov", "m4v", "mkv", "webm"];

interface Result {
  source: string;
  text: string;
  durationSecs?: number;
}

export function TranscribeScreen() {
  const dictation = useDictationState();
  const recording = dictation.phase === "recording" && dictation.destination === "screen";
  const transcribingRecording = dictation.phase === "transcribing" && dictation.destination === "screen";
  const now = useNow(500, recording);

  const [fileBusy, setFileBusy] = useState<string | null>(null);
  const [warming, setWarming] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [result, setResult] = useState<Result | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { copy, copiedKey } = useCopy();

  const busy = Boolean(fileBusy) || recording || transcribingRecording;

  const transcribe = async (path: string) => {
    const name = path.split(/[\\/]/).pop() ?? path;
    setFileBusy(name);
    setError(null);
    try {
      const { text, durationSecs } = await api.transcribeFile(path);
      setResult({ source: name, text, durationSecs });
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setFileBusy(null);
      setWarming(false);
    }
  };

  const chooseFile = async () => {
    const path = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Audio and video", extensions: EXTENSIONS }],
    });
    if (typeof path === "string") transcribe(path);
  };

  // Drag and drop comes from the native window, which reports file paths.
  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      const { type } = event.payload;
      if (type === "enter" || type === "over") setDragging(true);
      else if (type === "leave") setDragging(false);
      else if (type === "drop") {
        setDragging(false);
        const [path] = event.payload.paths;
        if (path && !busy) transcribe(path);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [busy]);

  useTauriEvent<boolean>("file-transcription-warming", ({ payload }) => setWarming(payload));
  useTauriEvent<DictationResult>("dictation-result", ({ payload }) => {
    if (payload.text) {
      setError(null);
      setResult({ source: "Recording", text: payload.text });
    } else if (payload.error) {
      setError(payload.error);
    }
  });

  return (
    <Page>
      <PageHeader
        title="Transcribe Audio"
        description="Turn a recording or video into text, or record straight into this window."
      />

      <Card
        padding="none"
        className={cn(
          "relative flex min-h-[320px] flex-col items-center justify-center border-2 border-dashed border-line-strong px-8 py-10 shadow-none text-center transition-colors",
          dragging && "border-accent bg-accent-soft",
        )}
      >
        {fileBusy || transcribingRecording ? (
          <>
            <Spinner size={28} className="text-accent-ink" />
            <h2 className="mt-5 type-section">{warming ? "Loading the model…" : "Transcribing…"}</h2>
            <p className="mt-1 type-small text-ink-secondary">{fileBusy ?? "Your recording"}</p>
          </>
        ) : recording ? (
          <>
            <span className="relative flex size-14 items-center justify-center">
              <span className="absolute inset-0 animate-ping rounded-full bg-danger/20" />
              <span className="relative flex size-14 items-center justify-center rounded-full bg-danger text-white">
                <Mic size={22} />
              </span>
            </span>
            <h2 className="mt-5 type-section tabular-nums">
              Recording · {formatClock((now - dictation.startedAtMs) / 1000)}
            </h2>
            <p className="mt-1 type-small text-ink-secondary">Speak now. Stop when you're done.</p>
            <Button variant="primary" icon={Square} className="mt-6" onClick={() => api.toggleDictation(false)}>
              Stop recording
            </Button>
          </>
        ) : (
          <>
            <IconTile icon={FileAudio} tone="brand" size="lg" />
            <h2 className="mt-5 type-section">{dragging ? "Drop to transcribe" : "Drop an audio or video file here"}</h2>
            <p className="mt-1 type-small text-ink-muted">MP3, M4A, WAV, FLAC, OGG, MP4, MOV and more</p>
            <div className="mt-6 flex items-center gap-3">
              <Button variant="accent" icon={Upload} onClick={chooseFile}>
                Choose file
              </Button>
              <span className="type-small text-ink-muted">or</span>
              <Button icon={Mic} onClick={() => api.toggleDictation(false)}>
                Record
              </Button>
            </div>
          </>
        )}
      </Card>

      {error && (
        <div className="mt-5">
          <Callout icon={CircleAlert} tone="danger" title="Couldn't transcribe that">
            {error}
          </Callout>
        </div>
      )}

      {result && (
        <Card padding="none" className="mt-5 animate-pop-in">
          <div className="flex items-center gap-3 px-5 pt-4 pb-3">
            <div className="min-w-0 flex-1">
              <h2 className="type-section">Transcription</h2>
              <p className="mt-0.5 truncate type-caption text-ink-muted">
                {result.source}
                {result.durationSecs !== undefined && ` · ${formatDuration(result.durationSecs)}`}
                {" · Saved to History"}
              </p>
            </div>
            <Button
              size="sm"
              icon={copiedKey === "result" ? Check : Copy}
              onClick={() => copy(result.text, "result")}
              className={cn(copiedKey === "result" && "text-success")}
            >
              {copiedKey === "result" ? "Copied" : "Copy"}
            </Button>
          </div>
          <div className="px-5 pb-5">
            <p
              data-selectable
              className="max-h-80 overflow-y-auto rounded-control bg-surface-sunken p-4 type-body-lg whitespace-pre-wrap text-ink"
            >
              {result.text}
            </p>
          </div>
        </Card>
      )}
    </Page>
  );
}
