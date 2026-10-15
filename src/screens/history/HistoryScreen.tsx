import { Check, ChevronDown, Copy, Cpu, FileText, FolderOpen, History, Pencil, Search, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
import { AudioPlayer } from "@/components/AudioPlayer";
import {
  Button,
  ConfirmDialog,
  EmptyState,
  GroupHeader,
  Hotkey,
  Page,
  PageHeader,
  TextArea,
  TextField,
  useToast,
} from "@/components/ui";
import { api, errorMessage, type HistoryItem } from "@/lib/api";
import { cn } from "@/lib/cn";
import { formatDay, formatDuration, formatTime, plural } from "@/lib/format";
import { useCopy, useHistory } from "@/lib/hooks";
import { useStore } from "@/lib/store";
import { useLearnedToast } from "@/lib/useLearnedToast";
import { groupHistory } from "./groupHistory";

export function HistoryScreen() {
  const { settings, status } = useStore();
  const { items } = useHistory();
  const toast = useToast();
  const [query, setQuery] = useState("");
  const [expanded, setExpanded] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<HistoryItem | null>(null);
  const [clearing, setClearing] = useState(false);

  const groups = useMemo(() => groupHistory(items ?? [], query), [items, query]);

  const showLearned = useLearnedToast();

  const run = (action: Promise<unknown>) => action.catch((e) => toast(errorMessage(e), "error"));
  /** Resolves once saved, so the card can close its editor; rejects to keep it open. */
  const saveEdit = async (item: HistoryItem, transcript: string) => {
    try {
      const learned = await api.editHistoryItem(item.id, transcript);
      if (learned.length > 0) showLearned(learned);
      else toast("Transcript saved");
    } catch (e) {
      toast(errorMessage(e), "error");
      throw e;
    }
  };
  const revealLabel = status.os === "macos" ? "Show in Finder" : status.os === "windows" ? "Show in Explorer" : "Show in folder";


  return (
    <Page>
      <PageHeader
        title="History"
        description={items && items.length > 0 ? plural(items.length, "transcription") : undefined}
        actions={
          items &&
          items.length > 0 && (
            <Button variant="ghost" icon={Trash2} onClick={() => setClearing(true)} className="-mr-3.5">
              Clear all
            </Button>
          )
        }
      />

      {items && items.length === 0 && (
        <EmptyState
          icon={FileText}
          title="No transcriptions yet"
          description={
            <span className="inline-flex flex-wrap items-center justify-center gap-1.5">
              {settings.recordingMode === "toggle" ? "Press" : "Hold"} <Hotkey value={settings.hotkey} /> in any app
              and start talking.
            </span>
          }
        />
      )}

      {items && items.length > 0 && (
        <>
          <TextField
            icon={Search}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search transcripts"
            aria-label="Search transcripts"
            className="mb-10"
          />
          {groups.length === 0 && (
            <p className="py-10 text-center type-body text-ink-muted">No transcripts match “{query.trim()}”.</p>
          )}
          <div className="flex flex-col gap-10">
            {groups.map(([day, dayItems]) => (
              <section key={day}>
                <GroupHeader
                  title={formatDay(day)}
                  action={<span className="type-caption text-ink-muted">{plural(dayItems.length, "transcription")}</span>}
                />
                <ol>
                  {dayItems.map((item) => (
                    <HistoryRow
                      key={item.id}
                      item={item}
                      expanded={expanded === item.id}
                      onToggle={() => setExpanded((id) => (id === item.id ? null : item.id))}
                      onDelete={() => setDeleting(item)}
                      onSaveEdit={(transcript) => saveEdit(item, transcript)}
                      learns={settings.learnCorrections}
                      onReveal={() => run(api.revealHistoryAudio(item.id))}
                      revealLabel={revealLabel}
                    />
                  ))}
                </ol>
              </section>
            ))}
          </div>
        </>
      )}

      <ConfirmDialog
        open={clearing}
        onClose={() => setClearing(false)}
        onConfirm={() => run(api.clearHistory())}
        title="Clear all history?"
        description="This removes your saved transcripts and recordings. Your statistics are kept."
        confirmLabel="Clear all"
      />
      <ConfirmDialog
        open={deleting !== null}
        onClose={() => setDeleting(null)}
        onConfirm={() => deleting && run(api.deleteHistoryItem(deleting.id))}
        title="Delete transcript?"
        description={
          deleting?.audioPath
            ? "This removes the transcript and its recording."
            : "This removes the transcript from your history."
        }
        confirmLabel="Delete"
      />
    </Page>
  );
}

/** Time, transcript and length on one line; opens in place to read, play, edit, copy or delete. */
function HistoryRow({
  item,
  expanded,
  onToggle,
  onDelete,
  onSaveEdit,
  learns,
  onReveal,
  revealLabel,
}: {
  item: HistoryItem;
  expanded: boolean;
  onToggle: () => void;
  onDelete: () => void;
  onSaveEdit: (transcript: string) => Promise<void>;
  /** Whether saving an edit teaches WordFlow the corrected words. */
  learns: boolean;
  onReveal: () => void;
  revealLabel: string;
}) {
  const { copy, copiedKey } = useCopy();
  const copied = copiedKey === item.id;
  const [draft, setDraft] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [showOriginal, setShowOriginal] = useState(false);
  const editing = expanded && draft !== null;
  const changed = draft !== null && draft.trim() !== "" && draft.trim() !== item.transcript;

  const save = () => {
    if (draft === null || !changed) return;
    setSaving(true);
    onSaveEdit(draft.trim())
      .then(() => setDraft(null))
      .catch(() => {})
      .finally(() => setSaving(false));
  };

  return (
    <li
      className={cn(
        "-mx-4 rounded-card px-4 transition-colors duration-150",
        // A hairline between rows, dropped around the open one, which lifts onto a soft surface instead.
        "not-last:after:block not-last:after:h-px not-last:after:bg-line",
        expanded ? "bg-surface/80 shadow-card after:opacity-0" : "hover:bg-hover/50",
      )}
    >
      <button
        type="button"
        onClick={onToggle}
        aria-expanded={expanded}
        className="grid w-full grid-cols-[72px_minmax(0,1fr)_auto] items-baseline gap-5 py-4 text-left"
      >
        <time className="type-small text-ink-muted tabular-nums">{formatTime(item.createdAt)}</time>
        {/* An open row shows the full, selectable transcript below instead. */}
        {expanded ? (
          <p className="type-small text-ink-muted tabular-nums">
            {plural(item.wordCount, "word")} · {formatDuration(item.durationSecs)}
          </p>
        ) : (
          <p className="line-clamp-2 type-feed text-ink">{item.transcript}</p>
        )}
        <span className="flex items-center gap-3 type-caption text-ink-muted tabular-nums">
          {!expanded && plural(item.wordCount, "word")}
          <ChevronDown
            size={16}
            className={cn("self-center transition-transform duration-200", expanded && "rotate-180")}
          />
        </span>
      </button>

      {expanded && (
        <div className="animate-fade-in pb-5 pl-[92px]">
          {editing ? (
            <div>
              <TextArea
                aria-label="Transcript"
                value={draft}
                onChange={(e) => setDraft(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) save();
                  if (e.key === "Escape") {
                    e.stopPropagation();
                    setDraft(null);
                  }
                }}
                rows={Math.min(8, Math.max(3, Math.ceil(item.transcript.length / 60)))}
                className="type-feed"
                autoFocus
              />
              <p className="mt-2 type-caption text-ink-muted">
                {learns
                  ? "Fix any misheard words. WordFlow remembers names and terms you correct, for next time."
                  : "Fix any misheard words. Learning from corrections is off in Settings."}
              </p>
              <div className="mt-3 flex items-center gap-2">
                <Button size="sm" variant="accent" icon={Check} loading={saving} disabled={!changed} onClick={save}>
                  Save
                </Button>
                <Button size="sm" variant="ghost" onClick={() => setDraft(null)} disabled={saving}>
                  Cancel
                </Button>
              </div>
            </div>
          ) : (
            <p data-selectable className="-mt-1 type-feed whitespace-pre-wrap text-ink">
              {item.transcript}
            </p>
          )}

          {!editing && showOriginal && item.original && (
            <div className="mt-4 border-l-2 border-line pl-3">
              <p className="type-caption text-ink-muted">Before AI cleanup</p>
              <p data-selectable className="mt-1 type-feed whitespace-pre-wrap text-ink-muted">
                {item.original}
              </p>
            </div>
          )}

          {!editing && item.audioPath && (
            <div className="mt-4">
              <AudioPlayer itemId={item.id} />
            </div>
          )}

          {!editing && (
            <div className="mt-4 -ml-3 flex flex-wrap items-center gap-1">
              <Button size="sm" variant="ghost" icon={Pencil} onClick={() => setDraft(item.transcript)}>
                Edit
              </Button>
              <Button
                size="sm"
                variant="ghost"
                icon={copied ? Check : Copy}
                onClick={() => copy(item.transcript, item.id)}
                className={cn(copied && "text-success hover:text-success")}
              >
                {copied ? "Copied" : "Copy"}
              </Button>
              {item.original && (
                <Button
                  size="sm"
                  variant="ghost"
                  icon={History}
                  aria-pressed={showOriginal}
                  onClick={() => setShowOriginal((shown) => !shown)}
                >
                  {showOriginal ? "Hide original" : "Show original"}
                </Button>
              )}
              {item.audioPath && (
                <Button size="sm" variant="ghost" icon={FolderOpen} onClick={onReveal}>
                  {revealLabel}
                </Button>
              )}
              <Button size="sm" variant="danger" icon={Trash2} onClick={onDelete}>
                Delete
              </Button>
              <span className="flex-1" />
              {item.model && (
                <span className="flex items-center gap-1.5 type-caption text-ink-muted">
                  <Cpu size={12} /> {item.model}
                </span>
              )}
            </div>
          )}
        </div>
      )}
    </li>
  );
}
