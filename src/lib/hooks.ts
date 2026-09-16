import { useCallback, useEffect, useState } from "react";
import {
  api,
  type DeviceReport,
  type DictationState,
  type EngineStatus,
  type HistoryItem,
  type Permission,
  type StatsEntry,
} from "./api";
import { useTauriEvent } from "./useTauriEvent";

/** History items and stats, kept fresh as dictations finish. */
export function useHistory() {
  const [items, setItems] = useState<HistoryItem[]>();
  const [stats, setStats] = useState<StatsEntry[]>([]);

  const refresh = useCallback(async () => {
    const [nextItems, nextStats] = await Promise.all([api.getHistory(), api.getStats()]);
    setItems(nextItems);
    setStats(nextStats);
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);
  useTauriEvent("history-changed", () => refresh());

  return { items, stats, refresh };
}

export function useEngineStatus() {
  const [status, setStatus] = useState<EngineStatus>({ loaded: null, loading: null });
  const refresh = useCallback(async () => setStatus(await api.getEngineStatus()), []);
  useEffect(() => {
    refresh();
  }, [refresh]);
  useTauriEvent("engine-changed", () => refresh());
  return status;
}

export function useDictationState() {
  const [state, setState] = useState<DictationState>({ phase: "idle" });
  useEffect(() => {
    api.getDictationState().then(setState);
  }, []);
  useTauriEvent<DictationState>("dictation-state", ({ payload }) => setState(payload));
  return state;
}

export function useDeviceReport() {
  const [report, setReport] = useState<DeviceReport>();
  useTauriEvent("settings-changed", () => api.getDeviceInfo().then(setReport));
  useEffect(() => {
    api.getDeviceInfo().then(setReport);
  }, []);
  return report;
}

/** Permission state, re-checked every second while mounted since users grant it in System Settings. */
export function usePermissions() {
  const [permissions, setPermissions] = useState<Permission[]>();
  useEffect(() => {
    let alive = true;
    const check = () => api.getPermissions().then((p) => alive && setPermissions(p));
    check();
    const timer = setInterval(check, 1000);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, []);
  return permissions;
}

/** Re-renders every `intervalMs` so relative times and live durations stay current. */
export function useNow(intervalMs = 1000, enabled = true) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (!enabled) return;
    const timer = setInterval(() => setNow(Date.now()), intervalMs);
    return () => clearInterval(timer);
  }, [intervalMs, enabled]);
  return now;
}

/** Copies text and reports success for a moment, for "Copied" button states. */
export function useCopy(resetMs = 1500) {
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const copy = useCallback(
    async (text: string, key = "default") => {
      await navigator.clipboard.writeText(text);
      setCopiedKey(key);
      setTimeout(() => setCopiedKey((k) => (k === key ? null : k)), resetMs);
    },
    [resetMs],
  );
  return { copy, copiedKey };
}
