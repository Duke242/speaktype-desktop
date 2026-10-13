import { ArrowRight, Pencil, Plus, TextQuote, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import {
  Button,
  ConfirmDialog,
  Dialog,
  EmptyState,
  IconButton,
  Section,
  Switch,
  TextArea,
  TextField,
  useToast,
} from "@/components/ui";
import { errorMessage, type Snippet } from "@/lib/api";
import { cn } from "@/lib/cn";
import { plural } from "@/lib/format";
import { useStore } from "@/lib/store";
import { Field } from "./Field";

/** Spoken phrases that expand into saved text. */
export function SnippetsTab() {
  const { settings, updateSettings } = useStore();
  const toast = useToast();
  const [editing, setEditing] = useState<Snippet | "new" | null>(null);
  const [deleting, setDeleting] = useState<Snippet | null>(null);
  const snippets = settings.snippets;

  const saveSnippets = (next: Snippet[]) =>
    updateSettings({ snippets: next }).catch((e) => toast(errorMessage(e), "error"));

  const saveSnippet = (snippet: Snippet) => {
    const exists = snippets.some((s) => s.id === snippet.id);
    saveSnippets(exists ? snippets.map((s) => (s.id === snippet.id ? snippet : s)) : [snippet, ...snippets]);
  };

  return (
    <>
      {snippets.length === 0 ? (
        <EmptyState
          icon={TextQuote}
          title="No snippets yet"
          description="Add a phrase like “my calendar link” and the text it stands for. Say the phrase while dictating to insert the text."
          action={
            <Button variant="primary" icon={Plus} onClick={() => setEditing("new")}>
              Add snippet
            </Button>
          }
        />
      ) : (
        <Section
          title={plural(snippets.length, "snippet")}
          action={
            <Button size="sm" variant="ghost" icon={Plus} onClick={() => setEditing("new")}>
              Add snippet
            </Button>
          }
        >
          {snippets.map((snippet) => (
            <SnippetRow
              key={snippet.id}
              snippet={snippet}
              onEdit={() => setEditing(snippet)}
              onDelete={() => setDeleting(snippet)}
              onToggle={(isEnabled) =>
                saveSnippets(snippets.map((s) => (s.id === snippet.id ? { ...s, isEnabled } : s)))
              }
            />
          ))}
        </Section>
      )}

      <SnippetEditor
        open={editing !== null}
        snippet={editing === "new" ? null : editing}
        taken={snippets.filter((s) => editing === "new" || s.id !== editing?.id).map((s) => normalize(s.trigger))}
        onClose={() => setEditing(null)}
        onSave={saveSnippet}
      />
      <ConfirmDialog
        open={deleting !== null}
        onClose={() => setDeleting(null)}
        onConfirm={() => deleting && saveSnippets(snippets.filter((s) => s.id !== deleting.id))}
        title="Delete snippet?"
        description={deleting && `“${deleting.trigger.trim()}” will no longer be expanded.`}
        confirmLabel="Delete"
      />
    </>
  );
}

/** Triggers match in any case and spacing, so "My  Email" is the same as "my email". */
function normalize(trigger: string) {
  return trigger.trim().replace(/\s+/g, " ").toLowerCase();
}

function SnippetRow({
  snippet,
  onEdit,
  onDelete,
  onToggle,
}: {
  snippet: Snippet;
  onEdit: () => void;
  onDelete: () => void;
  onToggle: (enabled: boolean) => void;
}) {
  return (
    <div className="group flex items-center gap-4 py-3.5">
      <button
        type="button"
        onClick={onEdit}
        className={cn("flex min-w-0 flex-1 items-center gap-3 text-left", !snippet.isEnabled && "opacity-45")}
      >
        <span className="shrink-0 type-label text-ink">{snippet.trigger.trim()}</span>
        <ArrowRight size={14} className="shrink-0 text-ink-muted" />
        <span className="truncate type-body text-ink-secondary">{snippet.expansion.replace(/\s*\n\s*/g, " ⏎ ")}</span>
      </button>
      <div className="flex items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
        <IconButton icon={Pencil} label="Edit snippet" size="sm" onClick={onEdit} />
        <IconButton icon={Trash2} label="Delete snippet" size="sm" tone="danger" onClick={onDelete} />
      </div>
      <Switch checked={snippet.isEnabled} onChange={onToggle} label={`Enable “${snippet.trigger.trim()}”`} />
    </div>
  );
}

function SnippetEditor({
  open,
  snippet,
  taken,
  onClose,
  onSave,
}: {
  open: boolean;
  snippet: Snippet | null;
  /** Triggers other snippets already use. */
  taken: string[];
  onClose: () => void;
  onSave: (snippet: Snippet) => void;
}) {
  const [trigger, setTrigger] = useState("");
  const [expansion, setExpansion] = useState("");

  useEffect(() => {
    if (!open) return;
    setTrigger(snippet?.trigger ?? "");
    setExpansion(snippet?.expansion ?? "");
  }, [open, snippet]);

  const duplicate = taken.includes(normalize(trigger));
  const valid = trigger.trim().length > 0 && expansion.length > 0 && !duplicate;
  const submit = () => {
    if (!valid) return;
    onSave({
      id: snippet?.id ?? crypto.randomUUID(),
      trigger: trigger.trim(),
      expansion,
      isEnabled: snippet?.isEnabled ?? true,
    });
    onClose();
  };

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={snippet ? "Edit snippet" : "New snippet"}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={!valid} onClick={submit}>
            {snippet ? "Save" : "Add snippet"}
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
        <Field
          label="When I say"
          hint={duplicate ? "Another snippet already uses this phrase." : "A short phrase you wouldn't say by accident."}
        >
          <TextField value={trigger} onChange={(e) => setTrigger(e.target.value)} placeholder="my calendar link" />
        </Field>
        <Field label="Insert" hint="The text to paste instead. It can span several lines.">
          <TextArea
            value={expansion}
            onChange={(e) => setExpansion(e.target.value)}
            placeholder="https://cal.com/jane"
            rows={4}
          />
        </Field>
        <button type="submit" hidden />
      </form>
    </Dialog>
  );
}
