import { FileDown, FolderSync, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Button, Section, SettingRow, useToast } from "@/components/ui";
import { api, errorMessage, type SyncStatus } from "@/lib/api";
import { cn } from "@/lib/cn";
import { useStore } from "@/lib/store";
import { useTauriEvent } from "@/lib/useTauriEvent";
import { describeLibraryImport, describeSync, folderName } from "./sharing";

/** Sharing the dictionary and snippets through a folder, and export and import. */
export function SharingSection() {
  const { settings, updateSettings } = useStore();
  const toast = useToast();
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [busy, setBusy] = useState<"sync" | "pick" | "export" | "import" | null>(null);
  const folder = settings.syncFolder;

  const refresh = useCallback(() => {
    api
      .getSyncStatus()
      .then(setStatus)
      .catch(() => {});
  }, []);

  // Sharing runs in the background, so check on it while this is open.
  useEffect(() => {
    if (!folder) return;
    refresh();
    const timer = setInterval(refresh, 15_000);
    return () => clearInterval(timer);
  }, [folder, refresh]);
  useTauriEvent("settings-changed", refresh);

  const run = async (what: NonNullable<typeof busy>, action: () => Promise<void>) => {
    setBusy(what);
    try {
      await action();
    } catch (error) {
      toast(errorMessage(error), "error");
    } finally {
      setBusy(null);
    }
  };

  const choose = () =>
    run("pick", async () => {
      const picked = await api.pickSyncFolder();
      if (!picked) return;
      await updateSettings({ syncFolder: picked });
      setStatus(await api.syncNow());
    });
  const syncNow = () => run("sync", async () => setStatus(await api.syncNow()));
  const exportFile = () =>
    run("export", async () => {
      const path = await api.exportLibrary();
      if (path) toast(`Saved to ${path}.`);
    });
  const importFile = () =>
    run("import", async () => {
      const result = await api.importLibrary();
      if (result) toast(describeLibraryImport(result));
    });

  return (
    <Section
      title="Sharing"
      description="Keep your dictionary and snippets the same on all your computers, or share them with your team."
      action={
        folder && (
          <Button size="sm" variant="ghost" icon={RefreshCw} loading={busy === "sync"} onClick={syncNow}>
            Sync now
          </Button>
        )
      }
    >
      {folder ? (
        <SettingRow
          icon={FolderSync}
          label={`Shared through “${folderName(folder)}”`}
          description={
            <>
              <span className="block truncate" title={folder}>
                {folder}
              </span>
              <span className={cn("block", status?.error && "text-danger")}>{describeSync(status)}</span>
            </>
          }
        >
          <div className="flex items-center gap-2">
            <Button size="sm" loading={busy === "pick"} onClick={choose}>
              Change…
            </Button>
            <Button size="sm" variant="ghost" onClick={() => run("pick", () => updateSettings({ syncFolder: "" }))}>
              Stop sharing
            </Button>
          </div>
        </SettingRow>
      ) : (
        <SettingRow
          icon={FolderSync}
          label="Share through a folder"
          description="Pick a folder that Dropbox, iCloud Drive, OneDrive or a shared drive keeps in step. Every computer or teammate that picks the same folder gets the same rules and snippets."
        >
          <Button loading={busy === "pick"} onClick={choose}>
            Choose folder…
          </Button>
        </SettingRow>
      )}
      <SettingRow
        icon={FileDown}
        label="Export or import"
        description="Save your rules and snippets to a file, or add them from one. Ones with the same phrase are replaced."
      >
        <div className="flex items-center gap-2">
          <Button size="sm" loading={busy === "import"} onClick={importFile}>
            Import…
          </Button>
          <Button size="sm" loading={busy === "export"} onClick={exportFile}>
            Export…
          </Button>
        </div>
      </SettingRow>
    </Section>
  );
}
