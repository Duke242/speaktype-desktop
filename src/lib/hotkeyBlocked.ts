import { useEffect } from "react";

/**
 * Why a single-key hotkey like fn does nothing: macOS hasn't given the app
 * Accessibility access. A dev build runs under the terminal that started it,
 * so that's the app macOS asks about.
 */
export function hotkeyBlockedMessage(keys: string) {
  const app = import.meta.env.DEV ? "the terminal running this dev build" : "Wordfall";
  return `${keys} won't start dictation until macOS allows it. Turn on ${app} in Privacy & Security → Accessibility.`;
}

/** Re-checks status every couple of seconds while blocked, so the warning clears once access is granted. */
export function useWhileHotkeyBlocked(blocked: boolean, refresh: () => Promise<void>) {
  useEffect(() => {
    if (!blocked) return;
    const timer = setInterval(refresh, 2000);
    return () => clearInterval(timer);
  }, [blocked, refresh]);
}
