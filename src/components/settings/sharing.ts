import type { LibraryImport, SyncStatus } from "@/lib/api";
import { formatRelative, plural } from "@/lib/format";

/** "Synced 5m ago", or what went wrong. */
export function describeSync(status: SyncStatus | null, now = Date.now()) {
  if (status?.error) return status.error;
  if (!status?.lastSyncedAt) return "Not synced yet.";
  const when = formatRelative(status.lastSyncedAt, now);
  return when === "just now" ? "Synced just now." : `Synced ${when}.`;
}

/** "Imported 2 rules and 1 snippet." */
export function describeLibraryImport({ rules, snippets }: LibraryImport) {
  const parts = [rules > 0 && plural(rules, "rule"), snippets > 0 && plural(snippets, "snippet")].filter(
    (part): part is string => Boolean(part),
  );
  return parts.length === 0 ? "Everything in that file is already here." : `Imported ${parts.join(" and ")}.`;
}

/** The last folder in a path, for a short label: "/Users/me/Dropbox/WordFlow" → "WordFlow". */
export function folderName(path: string) {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}
