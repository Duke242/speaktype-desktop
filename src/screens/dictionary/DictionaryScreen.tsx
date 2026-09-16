import { ArrowRight, BookA, Pencil, Plus, Trash2, Wand2 } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import {
  Badge,
  Button,
  Callout,
  Card,
  ConfirmDialog,
  Dialog,
  EmptyState,
  IconButton,
  Page,
  PageHeader,
  Switch,
  TextArea,
  TextField,
  useToast,
} from "@/components/ui";
import { errorMessage, type DictionaryEntry } from "@/lib/api";
import { cn } from "@/lib/cn";
import { useStore } from "@/lib/store";

export function DictionaryScreen() {
  const { settings, updateSettings } = useStore();
  const toast = useToast();
  const [editing, setEditing] = useState<DictionaryEntry | "new" | null>(null);
  const [deleting, setDeleting] = useState<DictionaryEntry | null>(null);
  const entries = settings.dictionary;

  const saveEntries = (dictionary: DictionaryEntry[]) =>
    updateSettings({ dictionary }).catch((e) => toast(errorMessage(e), "error"));

  const saveEntry = (entry: DictionaryEntry) => {
    const exists = entries.some((e) => e.id === entry.id);
    // New rules go first, so they run before older ones.
    saveEntries(exists ? entries.map((e) => (e.id === entry.id ? entry : e)) : [entry, ...entries]);
  };

  const addButton = (
    <Button variant="primary" icon={Plus} onClick={() => setEditing("new")}>
      Add rule
    </Button>
  );

  return (
    <Page>
      <PageHeader
        title="Dictionary"
        description={entries.length > 0 ? `${entries.length} ${entries.length === 1 ? "rule" : "rules"}` : undefined}
        actions={entries.length > 0 && addButton}
      />

      <div className="mb-6">
        <Callout icon={Wand2} tone="neutral" title="Turn what you say into the text you want">
          Say a trigger like “my email” and SpeakType types your real address. Or fix a name the model keeps
          mishearing. Everything runs offline.
        </Callout>
      </div>

      {entries.length === 0 ? (
        <Card>
          <EmptyState
            icon={BookA}
            title="No rules yet"
            description="Add your first rule to replace a spoken phrase with any text."
            action={addButton}
          />
        </Card>
      ) : (
        <div className="flex flex-col gap-2">
          {entries.map((entry) => (
            <RuleRow
              key={entry.id}
              entry={entry}
              onEdit={() => setEditing(entry)}
              onDelete={() => setDeleting(entry)}
              onToggle={(isEnabled) => saveEntries(entries.map((e) => (e.id === entry.id ? { ...e, isEnabled } : e)))}
            />
          ))}
        </div>
      )}

      <RuleEditor
        open={editing !== null}
        entry={editing === "new" ? null : editing}
        onClose={() => setEditing(null)}
        onSave={saveEntry}
      />
      <ConfirmDialog
        open={deleting !== null}
        onClose={() => setDeleting(null)}
        onConfirm={() => deleting && saveEntries(entries.filter((e) => e.id !== deleting.id))}
        title="Delete rule?"
        description={deleting && `“${deleting.trigger.trim()}” will no longer be replaced.`}
        confirmLabel="Delete"
      />
    </Page>
  );
}

function RuleRow({
  entry,
  onEdit,
  onDelete,
  onToggle,
}: {
  entry: DictionaryEntry;
  onEdit: () => void;
  onDelete: () => void;
  onToggle: (enabled: boolean) => void;
}) {
  return (
    <Card padding="none" interactive className="group flex items-center gap-4 px-5 py-3.5">
      <button
        type="button"
        onClick={onEdit}
        className={cn("flex min-w-0 flex-1 items-center gap-3 text-left", !entry.isEnabled && "opacity-45")}
      >
        <span className="truncate type-label text-ink">{entry.trigger.trim()}</span>
        <ArrowRight size={14} className="shrink-0 text-ink-muted" />
        {entry.replacement ? (
          <span className="truncate type-body text-ink-secondary">{entry.replacement}</span>
        ) : (
          <span className="type-body text-ink-muted italic">removed</span>
        )}
        {!entry.matchWholeWord && <Badge className="ml-1">Partial match</Badge>}
      </button>
      <div className="flex items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
        <IconButton icon={Pencil} label="Edit rule" size="sm" onClick={onEdit} />
        <IconButton icon={Trash2} label="Delete rule" size="sm" tone="danger" onClick={onDelete} />
      </div>
      <Switch checked={entry.isEnabled} onChange={onToggle} label={`Enable “${entry.trigger.trim()}”`} />
    </Card>
  );
}

function RuleEditor({
  open,
  entry,
  onClose,
  onSave,
}: {
  open: boolean;
  entry: DictionaryEntry | null;
  onClose: () => void;
  onSave: (entry: DictionaryEntry) => void;
}) {
  const [trigger, setTrigger] = useState("");
  const [replacement, setReplacement] = useState("");
  const [wholeWord, setWholeWord] = useState(true);

  useEffect(() => {
    if (!open) return;
    setTrigger(entry?.trigger ?? "");
    setReplacement(entry?.replacement ?? "");
    setWholeWord(entry?.matchWholeWord ?? true);
  }, [open, entry]);

  const valid = trigger.trim().length > 0;
  const submit = () => {
    if (!valid) return;
    onSave({
      id: entry?.id ?? crypto.randomUUID(),
      trigger: trigger.trim(),
      replacement,
      isEnabled: entry?.isEnabled ?? true,
      matchWholeWord: wholeWord,
    });
    onClose();
  };

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={entry ? "Edit rule" : "New rule"}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={!valid} onClick={submit}>
            {entry ? "Save" : "Add rule"}
          </Button>
        </>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
        className="flex flex-col gap-5"
      >
        <Field label="When I say" hint="The word or phrase to listen for.">
          <TextField value={trigger} onChange={(e) => setTrigger(e.target.value)} placeholder="my email" />
        </Field>
        <Field label="Replace with" hint="Any text. Leave it empty to remove the phrase.">
          <TextArea
            value={replacement}
            onChange={(e) => setReplacement(e.target.value)}
            placeholder="jane.doe@example.com"
            rows={3}
          />
        </Field>
        <div className="flex items-center gap-4 border-t border-line-subtle pt-4">
          <span className="flex-1">
            <span className="block type-label">Match whole words only</span>
            <span className="block type-small text-ink-secondary">Won't fire inside longer words.</span>
          </span>
          <Switch checked={wholeWord} onChange={setWholeWord} label="Match whole words only" />
        </div>
        <button type="submit" hidden />
      </form>
    </Dialog>
  );
}

function Field({ label, hint, children }: { label: string; hint: string; children: ReactNode }) {
  return (
    <div>
      <div className="mb-2">
        <div className="type-label">{label}</div>
        <div className="type-small text-ink-secondary">{hint}</div>
      </div>
      {children}
    </div>
  );
}
