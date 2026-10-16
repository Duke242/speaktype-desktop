import {
  ArrowLeft,
  ArrowRight,
  Check,
  CircleAlert,
  CircleCheck,
  Copy,
  FileDown,
  FolderOpen,
  Pencil,
  Sparkles,
  Trash2,
} from "lucide-react";
import { useMemo, useState } from "react";
import type { Route } from "@/app/routes";
import {
  Button,
  Callout,
  ConfirmDialog,
  EmptyState,
  GroupHeader,
  IconButton,
  Page,
  Spinner,
  TextField,
  useToast,
} from "@/components/ui";
import { api, errorMessage, type Meeting } from "@/lib/api";
import { cn } from "@/lib/cn";
import { formatDuration, plural } from "@/lib/format";
import { useCopy } from "@/lib/hooks";
import { useStore } from "@/lib/store";
import { showSettingsTab } from "@/screens/settings/SettingsScreen";
import { formatMeetingDate, formatTimestamp, isLabelled, meetingMarkdown, transcriptBlocks } from "./transcript";
import { useMeeting, useMeetingSetup } from "./useMeetings";

export function MeetingDetail({
  id,
  onBack,
  onNavigate,
}: {
  id: string;
  onBack: () => void;
  onNavigate: (route: Route) => void;
}) {
  const { status } = useStore();
  const { meeting, setMeeting } = useMeeting(id);
  const toast = useToast();
  const { copy, copiedKey } = useCopy();
  const [deleting, setDeleting] = useState(false);
  const run = (action: Promise<unknown>) => action.catch((e) => toast(errorMessage(e), "error"));
  const revealLabel = status.os === "macos" ? "Show in Finder" : status.os === "windows" ? "Show in Explorer" : "Show in folder";

  const back = (
    <Button variant="ghost" size="sm" icon={ArrowLeft} onClick={onBack} className="-ml-3 mb-6">
      Meetings
    </Button>
  );

  if (meeting === undefined) return <Page>{back}</Page>;
  if (meeting === null) {
    return (
      <Page>
        {back}
        <EmptyState icon={CircleAlert} title="This meeting was deleted" />
      </Page>
    );
  }

  const done = meeting.status === "done";
  const markdown = () => meetingMarkdown(meeting);
  const exportMarkdown = () =>
    api
      .exportMeetingMarkdown(meeting.id, markdown())
      .then(() => toast("Saved to your Downloads folder"))
      .catch((e) => toast(errorMessage(e), "error"));

  return (
    <Page>
      {back}
      <Title meeting={meeting} onRename={(title) => setMeeting({ ...meeting, title })} />
      <p className="mt-2 type-body-lg text-ink-secondary">
        {formatMeetingDate(meeting.createdAt)} · {formatDuration(meeting.durationSecs)} ·{" "}
        {plural(meeting.segments.reduce((n, s) => n + s.text.split(/\s+/).filter(Boolean).length, 0), "word")}
      </p>

      <div className="mt-5 mb-10 -ml-3 flex flex-wrap items-center gap-1">
        <Button
          size="sm"
          variant="ghost"
          icon={copiedKey === "markdown" ? Check : Copy}
          onClick={() => copy(markdown(), "markdown")}
          className={cn(copiedKey === "markdown" && "text-success hover:text-success")}
        >
          {copiedKey === "markdown" ? "Copied" : "Copy as Markdown"}
        </Button>
        <Button size="sm" variant="ghost" icon={FileDown} onClick={exportMarkdown}>
          Save as Markdown
        </Button>
        {meeting.audioPath && (
          <Button size="sm" variant="ghost" icon={FolderOpen} onClick={() => run(api.revealMeetingAudio(meeting.id))}>
            {revealLabel}
          </Button>
        )}
        {done && (
          <Button size="sm" variant="danger" icon={Trash2} onClick={() => setDeleting(true)}>
            Delete
          </Button>
        )}
      </div>

      <Notes meeting={meeting} onNavigate={onNavigate} onWritten={setMeeting} />
      <Transcript meeting={meeting} />

      <ConfirmDialog
        open={deleting}
        onClose={() => setDeleting(false)}
        onConfirm={() => run(api.deleteMeeting(meeting.id).then(onBack))}
        title="Delete meeting?"
        description="This removes the transcript, the notes and the recording."
        confirmLabel="Delete"
      />
    </Page>
  );
}

function Title({ meeting, onRename }: { meeting: Meeting; onRename: (title: string) => void }) {
  const toast = useToast();
  const [draft, setDraft] = useState<string | null>(null);

  const save = () => {
    const title = draft?.trim();
    setDraft(null);
    if (!title || title === meeting.title) return;
    api
      .renameMeeting(meeting.id, title)
      .then(() => onRename(title))
      .catch((e) => toast(errorMessage(e), "error"));
  };

  if (draft !== null) {
    return (
      <TextField
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={save}
        onKeyDown={(e) => {
          if (e.key === "Enter") save();
          if (e.key === "Escape") {
            e.stopPropagation();
            setDraft(null);
          }
        }}
        aria-label="Meeting title"
        autoFocus
        className="max-w-[560px]"
      />
    );
  }
  return (
    <div className="group flex items-center gap-2">
      <h1 className="type-title">{meeting.title}</h1>
      <IconButton
        size="sm"
        icon={Pencil}
        label="Rename"
        onClick={() => setDraft(meeting.title)}
        className="opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
      />
    </div>
  );
}

function Notes({
  meeting,
  onNavigate,
  onWritten,
}: {
  meeting: Meeting;
  onNavigate: (route: Route) => void;
  onWritten: (meeting: Meeting) => void;
}) {
  const setup = useMeetingSetup();
  const [requested, setRequested] = useState(false);
  const toast = useToast();
  const writing = meeting.summarizing || requested;
  const notes = meeting.notes;
  const hasTranscript = meeting.segments.length > 0;

  const write = () => {
    setRequested(true);
    api
      .summarizeMeeting(meeting.id)
      .then(onWritten)
      .catch((e) => toast(errorMessage(e), "error"))
      .finally(() => setRequested(false));
  };

  const action =
    setup?.notesAvailable && hasTranscript && meeting.status === "done" && !writing ? (
      <Button size="sm" variant="ghost" icon={Sparkles} onClick={write}>
        {notes ? "Write again" : "Write notes"}
      </Button>
    ) : undefined;

  return (
    <section className="mb-12">
      <GroupHeader title="Notes" action={action} />
      <div className="pt-5">
        {writing ? (
          <div className="flex items-center gap-3 type-body text-ink-secondary">
            <Spinner size={16} />
            Writing notes. A long meeting can take a few minutes on this computer.
          </div>
        ) : notes ? (
          <NotesBody notes={notes} />
        ) : setup && !setup.notesAvailable ? (
          <Callout
            icon={Sparkles}
            tone="accent"
            title="Notes need an AI model"
            action={
              <Button
                size="sm"
                variant="ghost"
                icon={ArrowRight}
                onClick={() => {
                  showSettingsTab("ai");
                  onNavigate("settings");
                }}
              >
                Open Settings
              </Button>
            }
          >
            Choose an AI cleanup model in Settings, built in or on a local server, and WordFlow writes a summary, action items and
            open questions for each meeting.
          </Callout>
        ) : !hasTranscript ? (
          <p className="type-body text-ink-muted">Notes can be written once there's a transcript.</p>
        ) : meeting.status !== "done" ? (
          <p className="type-body text-ink-muted">Notes can be written once the meeting ends.</p>
        ) : (
          <p className="type-body text-ink-muted">No notes yet.</p>
        )}
        {!writing && meeting.notesError && (
          <div className="mt-4">
            <Callout icon={CircleAlert} tone="danger" title="Couldn't write notes">
              {meeting.notesError}
            </Callout>
          </div>
        )}
      </div>
    </section>
  );
}

function NotesBody({ notes }: { notes: NonNullable<Meeting["notes"]> }) {
  return (
    <div data-selectable className="flex flex-col gap-8">
      <p className="type-feed whitespace-pre-wrap text-ink">{notes.summary || "No summary."}</p>
      <NoteList title="Action items" items={notes.actionItems} empty="No action items." icon />
      <NoteList title="Open questions" items={notes.openQuestions} empty="No open questions." />
    </div>
  );
}

function NoteList({ title, items, empty, icon }: { title: string; items: string[]; empty: string; icon?: boolean }) {
  return (
    <div>
      <h3 className="mb-2 type-label text-ink">{title}</h3>
      {items.length === 0 ? (
        <p className="type-body text-ink-muted">{empty}</p>
      ) : (
        <ul className="flex flex-col gap-2">
          {items.map((item, i) => (
            <li key={i} className="flex items-start gap-2.5 type-body text-ink">
              {icon ? (
                <CircleCheck size={16} className="mt-0.5 shrink-0 text-accent-ink" />
              ) : (
                <span className="mt-2 size-1.5 shrink-0 rounded-full bg-ink-muted" />
              )}
              {item}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function Transcript({ meeting }: { meeting: Meeting }) {
  const blocks = useMemo(() => transcriptBlocks(meeting.segments, isLabelled(meeting)), [meeting]);
  return (
    <section>
      <GroupHeader
        title="Transcript"
        action={meeting.model ? <span className="type-caption text-ink-muted">{meeting.model}</span> : undefined}
      />
      {blocks.length === 0 ? (
        <p className="pt-5 type-body text-ink-muted">Nothing was transcribed.</p>
      ) : (
        <ol data-selectable className="flex flex-col gap-4 pt-5">
          {blocks.map((block) => (
            <li key={`${block.startSecs}-${block.speaker}`} className="grid grid-cols-[72px_minmax(0,1fr)] gap-5">
              <span className="type-small text-ink-muted tabular-nums">{formatTimestamp(block.startSecs)}</span>
              <div>
                {block.speaker && <div className="type-caption font-medium text-accent-ink">{block.speaker}</div>}
                <p className="type-feed text-ink">{block.text}</p>
              </div>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
