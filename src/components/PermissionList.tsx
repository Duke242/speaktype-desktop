import { Hand, Mic, ShieldCheck, type LucideIcon } from "lucide-react";
import { useEffect } from "react";
import { Badge, Button, Card, SettingRow } from "@/components/ui";
import { api, type PermissionKind } from "@/lib/api";
import { usePermissions } from "@/lib/hooks";

const COPY: Record<PermissionKind, { icon: LucideIcon; title: string; description: string }> = {
  microphone: { icon: Mic, title: "Microphone", description: "To hear and transcribe your voice." },
  accessibility: { icon: Hand, title: "Accessibility", description: "To paste transcribed text into any app." },
};

async function allow(kind: PermissionKind) {
  await api.requestPermission(kind);
  // macOS only prompts once. If access is still missing, open System Settings.
  setTimeout(async () => {
    const permissions = await api.getPermissions();
    if (kind === "microphone" && !permissions.find((p) => p.kind === kind)?.granted) {
      api.openPermissionSettings(kind);
    }
  }, 800);
}

/** The OS permissions SpeakType needs, with a way to grant each. Re-checks every second. */
export function PermissionList({ onChange }: { onChange?: (allGranted: boolean) => void }) {
  const permissions = usePermissions();
  const allGranted = permissions?.every((p) => p.granted);
  useEffect(() => {
    if (allGranted !== undefined) onChange?.(allGranted);
  }, [allGranted, onChange]);

  return (
    <Card padding="none" className="divide-y divide-line-subtle">
      {(permissions ?? []).map(({ kind, granted }) => {
        const copy = COPY[kind];
        return (
          <SettingRow key={kind} icon={copy.icon} label={copy.title} description={copy.description}>
            {granted ? (
              <Badge tone="success" icon={ShieldCheck}>
                Allowed
              </Badge>
            ) : (
              <Button variant="primary" size="sm" onClick={() => allow(kind)}>
                Allow
              </Button>
            )}
          </SettingRow>
        );
      })}
    </Card>
  );
}
