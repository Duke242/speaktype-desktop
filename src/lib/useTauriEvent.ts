import { listen, type EventCallback } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";

/** Subscribes to a backend event for the lifetime of the component. */
export function useTauriEvent<T>(event: string, handler: EventCallback<T>) {
  const latest = useRef(handler);
  latest.current = handler;

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    listen<T>(event, (e) => latest.current(e)).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [event]);
}
