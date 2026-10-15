import { useEffect } from "react";

/**
 * Why a single-key hotkey like fn does nothing: macOS hasn't given the app
 * Accessibility access. A dev build runs under the terminal that started it,
 * so that's the app macOS asks about.
 *
 * An update changes the app's signature, so an old grant can still show as on
 * while macOS refuses it. Only removing and re-adding the app clears that.
 */
export function hotkeyBlockedMessage(keys: string) {
  const app = import.meta.env.DEV ? "the terminal running this dev build" : "WordFlow";
  const stale = import.meta.env.DEV
    ? ""
    : " Already on? An update can reset it: remove WordFlow with −, add it back with +, then reopen WordFlow.";
  return `${keys} won't start dictation until macOS allows it. Turn on ${app} in Privacy & Security → Accessibility.${stale}`;
}

/** Re-checks status every couple of seconds while blocked, so the warning clears once access is granted. */
export function useWhileHotkeyBlocked(blocked: boolean, refresh: () => Promise<void>) {
  useEffect(() => {
    if (!blocked) return;
    const timer = setInterval(refresh, 2000);
    return () => clearInterval(timer);
  }, [blocked, refresh]);
}
