import { openUrl } from "@tauri-apps/plugin-opener";
import { Download } from "lucide-react";
import { Button, Dialog } from "@/components/ui";
import type { UpdateInfo } from "@/lib/api";

export function UpdateDialog({ update, onClose }: { update: UpdateInfo | null; onClose: () => void }) {
  const notes = update?.notes
    .split("\n")
    .map((line) => line.replace(/^\s*[-*•]\s*/, "").trim())
    .filter((line) => line && !line.startsWith("#"));

  return (
    <Dialog
      open={Boolean(update)}
      onClose={onClose}
      width={520}
      title="A new version of SpeakType is available"
      description={update && `SpeakType ${update.latestVersion} is out. You have ${update.currentVersion}.`}
      footer={
        <>
          <Button onClick={onClose}>Remind me later</Button>
          <Button
            variant="primary"
            icon={Download}
            onClick={() => {
              if (update) openUrl(update.url);
              onClose();
            }}
          >
            Get the update
          </Button>
        </>
      }
    >
      {notes && notes.length > 0 && (
        <div className="rounded-card border border-line-subtle bg-surface-sunken p-4">
          <p className="mb-2 type-overline text-ink-muted">What's new</p>
          <ul data-selectable className="max-h-56 space-y-1.5 overflow-y-auto">
            {notes.map((note, i) => (
              <li key={i} className="flex gap-2 type-small text-ink-secondary">
                <span className="mt-[7px] size-1 shrink-0 rounded-full bg-accent" />
                {note}
              </li>
            ))}
          </ul>
        </div>
      )}
    </Dialog>
  );
}
