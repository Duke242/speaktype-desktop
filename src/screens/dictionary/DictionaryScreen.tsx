import { ArrowRight, BookA, Lightbulb, Pencil, Plus, TextQuote, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import {
  Badge,
  Button,
  ConfirmDialog,
  Dialog,
  EmptyState,
  IconButton,
  Section,
  SegmentedControl,
  Page,
  PageHeader,
  Switch,
  TextArea,
  TextField,
  useToast,
} from "@/components/ui";
import { errorMessage, type DictionaryEntry } from "@/lib/api";
import { cn } from "@/lib/cn";
import { plural } from "@/lib/format";
import { useStore } from "@/lib/store";
import { Field } from "./Field";
import { SnippetsTab } from "./SnippetsTab";

type Tab = "replacements" | "snippets";

export function DictionaryScreen() {
  const [tab, setTab] = useState<Tab>("replacements");
  return (
    <Page>
      <PageHeader
        title="Dictionary"
        description={
          tab === "replacements"
            ? "Turn what you say into the text you want. Fix a name the model keeps mishearing, or remove a word you never mean. Words you correct are added here too, marked Learned."
            : "Say a short phrase and get longer text, like your calendar link or address. Everything runs offline."
        }
        actions={
          <SegmentedControl
            value={tab}
            onChange={setTab}
            options={[
              { value: "replacements", label: "Replacements", icon: BookA },
              { value: "snippets", label: "Snippets", icon: TextQuote },
            ]}
            className="w-[270px]"
          />
        }
      />
      {tab === "replacements" ? <ReplacementsTab /> : <SnippetsTab />}
    </Page>
  );
}

function ReplacementsTab() {
  const { settings, updateSettings } = useStore();
  const toast = useToast();
  const [editing, setEditing] = useState<DictionaryEntry | "new" | null>(null);
  const [deleting, setDeleting] = useState<DictionaryEntry | null>(null);
  const entries = settings.dictionary;
  const learnedCount = entries.filter((e) => e.learned).length;

  const saveEntries = (dictionary: DictionaryEntry[]) =>
    updateSettings({ dictionary }).catch((e) => toast(errorMessage(e), "error"));

  const saveEntry = (entry: DictionaryEntry) => {
    const exists = entries.some((e) => e.id === entry.id);
    // New rules go first, so they run before older ones.
    saveEntries(exists ? entries.map((e) => (e.id === entry.id ? entry : e)) : [entry, ...entries]);
  };

  return (
    <>
      {entries.length === 0 ? (
        <EmptyState
          icon={BookA}
          title="No rules yet"
          description="Add your first rule to replace a spoken phrase with any text. Everything runs offline."
          action={
            <Button variant="primary" icon={Plus} onClick={() => setEditing("new")}>
              Add rule
            </Button>
          }
        />
      ) : (
        <Section
          title={plural(entries.length, "rule")}
          action={
            <span className="flex items-center gap-3">
              {learnedCount > 0 && <span className="type-caption text-ink-muted">{learnedCount} learned</span>}
              <Button size="sm" variant="ghost" icon={Plus} onClick={() => setEditing("new")}>
                Add rule
              </Button>
            </span>
          }
        >
          {entries.map((entry) => (
            <RuleRow
              key={entry.id}
              entry={entry}
              onEdit={() => setEditing(entry)}
              onDelete={() => setDeleting(entry)}
              onToggle={(isEnabled) => saveEntries(entries.map((e) => (e.id === entry.id ? { ...e, isEnabled } : e)))}
            />
          ))}
        </Section>
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
    </>
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
    <div className="group flex items-center gap-4 py-3.5">
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
        {entry.learned && (
          <Badge tone="accent" icon={Lightbulb} className="ml-1">
            Learned
          </Badge>
        )}
      </button>
      <div className="flex items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
        <IconButton icon={Pencil} label="Edit rule" size="sm" onClick={onEdit} />
        <IconButton icon={Trash2} label="Delete rule" size="sm" tone="danger" onClick={onDelete} />
      </div>
      <Switch checked={entry.isEnabled} onChange={onToggle} label={`Enable “${entry.trigger.trim()}”`} />
    </div>
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
