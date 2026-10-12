import {
  Check,
  ChevronDown,
  Clock,
  Copy,
  Cpu,
  FileText,
  FolderOpen,
  Pencil,
  Search,
  Timer,
  Trash2,
  Type,
} from "lucide-react";
import { useMemo, useState } from "react";
import { AudioPlayer } from "@/components/AudioPlayer";
import {
  Button,
  Card,
  ConfirmDialog,
  EmptyState,
  Hotkey,
  Meta,
  Page,
  PageHeader,
  TextArea,
  TextField,
  useToast,
} from "@/components/ui";
import { api, errorMessage, type HistoryItem } from "@/lib/api";
import { cn } from "@/lib/cn";
import { formatDay, formatDuration, formatNumber, formatTime } from "@/lib/format";
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
        description={items && items.length > 0 ? `${formatNumber(items.length)} transcriptions` : undefined}
        actions={
          items &&
          items.length > 0 && (
            <Button variant="ghost" icon={Trash2} onClick={() => setClearing(true)}>
              Clear all
            </Button>
          )
        }
      />

      {items && items.length === 0 && (
        <Card>
          <EmptyState
            icon={FileText}
            title="No transcriptions yet"
            description={
              <span className="inline-flex flex-wrap items-center justify-center gap-1.5">
                {settings.recordingMode === "hold" ? "Hold" : "Press"} <Hotkey value={settings.hotkey} /> in any app
                and start talking.
              </span>
            }
          />
        </Card>
      )}

      {items && items.length > 0 && (
        <>
          <TextField
            icon={Search}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search transcripts"
            className="mb-6"
          />
          {groups.length === 0 && (
            <p className="py-10 text-center type-body text-ink-muted">No transcripts match “{query.trim()}”.</p>
          )}
          <div className="flex flex-col gap-7">
            {groups.map(([day, dayItems]) => (
              <section key={day}>
                <h2 className="mb-2.5 px-1 type-overline text-ink-muted">{formatDay(day)}</h2>
                <div className="flex flex-col gap-2">
                  {dayItems.map((item) => (
                    <HistoryCard
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
                </div>
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

function HistoryCard({
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
  /** Whether saving an edit teaches Wordfall the corrected words. */
  learns: boolean;
  onReveal: () => void;
  revealLabel: string;
}) {
  const { copy, copiedKey } = useCopy();
  const copied = copiedKey === item.id;
  const [draft, setDraft] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
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
    <Card padding="none" interactive className={cn(expanded && "border-line-strong")}>
      <button type="button" onClick={onToggle} aria-expanded={expanded} className="flex w-full items-start gap-4 p-4 text-left">
        <div className="min-w-0 flex-1">
          {/* Expanded cards show the full, selectable transcript below instead. */}
          {!expanded && <p className="mb-1.5 line-clamp-2 type-body text-ink">{item.transcript}</p>}
          <div className="flex items-center gap-4 type-caption text-ink-muted">
            <Meta icon={Clock}>{formatTime(item.createdAt)}</Meta>
            <Meta icon={Type}>
              {item.wordCount} {item.wordCount === 1 ? "word" : "words"}
            </Meta>
            <Meta icon={Timer}>{formatDuration(item.durationSecs)}</Meta>
          </div>
        </div>
        <ChevronDown
          size={16}
          className={cn("mt-1 shrink-0 text-ink-muted transition-transform duration-200", expanded && "rotate-180")}
        />
      </button>

      {expanded && (
        <div className="animate-fade-in px-4 pb-4">
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
                rows={Math.min(8, Math.max(3, Math.ceil(item.transcript.length / 70)))}
                className="type-body-lg"
              />
              <p className="mt-2 type-caption text-ink-muted">
                {learns
                  ? "Fix any misheard words. Wordfall remembers names and terms you correct, for next time."
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
            <p data-selectable className="type-body-lg whitespace-pre-wrap text-ink">
              {item.transcript}
            </p>
          )}

          {!editing && item.audioPath && (
            <div className="mt-4">
              <AudioPlayer itemId={item.id} />
            </div>
          )}

          {!editing && (
          <div className="mt-4 flex items-center gap-2">
            <Button size="sm" icon={Pencil} onClick={() => setDraft(item.transcript)}>
              Edit
            </Button>
            <Button size="sm" icon={copied ? Check : Copy} onClick={() => copy(item.transcript, item.id)} className={cn(copied && "text-success")}>
              {copied ? "Copied" : "Copy"}
            </Button>
            {item.audioPath && (
              <Button size="sm" icon={FolderOpen} onClick={onReveal}>
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
    </Card>
  );
}
