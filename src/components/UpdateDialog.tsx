import { openUrl } from "@tauri-apps/plugin-opener";
import { Download } from "lucide-react";
import { useState } from "react";
import { Button, Dialog, ProgressBar, useToast } from "@/components/ui";
import { api, errorMessage, type UpdateInfo, type UpdateProgress } from "@/lib/api";
import { useTauriEvent } from "@/lib/useTauriEvent";

export function UpdateDialog({ update, onClose }: { update: UpdateInfo | null; onClose: () => void }) {
  const toast = useToast();
  // Set while installing: how much of the download has arrived, 0..1.
  const [progress, setProgress] = useState<number | null>(null);
  useTauriEvent<UpdateProgress>("update-progress", ({ payload }) => {
    if (payload.total) setProgress(payload.downloaded / payload.total);
  });

  // Wordfall restarts into the new version once it's installed.
  const install = async () => {
    setProgress(0);
    try {
      await api.installUpdate();
    } catch (error) {
      setProgress(null);
      toast(errorMessage(error), "error");
    }
  };
  const installing = progress !== null;
  // Closing mid-download would hide the progress, not stop the install.
  const close = () => !installing && onClose();

  const notes = update?.notes
    .split("\n")
    .map((line) => line.replace(/^\s*[-*•]\s*/, "").trim())
    .filter((line) => line && !line.startsWith("#"));

  return (
    <Dialog
      open={Boolean(update)}
      onClose={close}
      width={520}
      title="A new version of Wordfall is available"
      description={update && `Wordfall ${update.latestVersion} is out. You have ${update.currentVersion}.`}
      footer={
        <>
          <Button onClick={close} disabled={installing}>
            Remind me later
          </Button>
          {update?.installable ? (
            <Button variant="primary" icon={Download} loading={installing} onClick={install}>
              {installing ? "Installing…" : "Install and restart"}
            </Button>
          ) : (
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
          )}
        </>
      }
    >
      {installing && <ProgressBar value={progress} className="mb-4" />}
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
