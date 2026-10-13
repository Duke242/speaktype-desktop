import { useCallback, useEffect, useState } from "react";
import {
  api,
  type Meeting,
  type MeetingListItem,
  type MeetingSegmentEvent,
  type MeetingSetup,
  type MeetingState,
} from "@/lib/api";
import { useTauriEvent } from "@/lib/useTauriEvent";
import { insertSegment } from "./transcript";

export function useMeetingState() {
  const [state, setState] = useState<MeetingState>({ phase: "idle" });
  useEffect(() => {
    api.getMeetingState().then(setState);
  }, []);
  useTauriEvent<MeetingState>("meeting-state", ({ payload }) => setState(payload));
  return state;
}

/** Saved meetings matching `query`, kept fresh as meetings change. */
export function useMeetingList(query: string) {
  const [items, setItems] = useState<MeetingListItem[]>();
  const refresh = useCallback(() => api.listMeetings(query.trim()).then(setItems), [query]);
  useEffect(() => {
    // Searching reads every saved transcript, so wait for a pause in typing.
    const timer = setTimeout(refresh, query ? 200 : 0);
    return () => clearTimeout(timer);
  }, [refresh, query]);
  useTauriEvent("meetings-changed", () => refresh());
  return items;
}

/** One meeting, with segments added as they're transcribed. `null` once it's gone. */
export function useMeeting(id: string) {
  const [meeting, setMeeting] = useState<Meeting | null>();
  const [error, setError] = useState<string>();
  const refresh = useCallback(
    () =>
      api
        .getMeeting(id)
        .then((m) => {
          setMeeting(m);
          setError(undefined);
        })
        .catch((e) => {
          setMeeting(null);
          setError(String(e));
        }),
    [id],
  );
  useEffect(() => {
    refresh();
  }, [refresh]);
  useTauriEvent("meetings-changed", () => refresh());
  useTauriEvent<MeetingSegmentEvent>("meeting-segment", ({ payload }) => {
    if (payload.meetingId !== id) return;
    setMeeting((m) => m && { ...m, segments: insertSegment(m.segments, payload.segment) });
  });
  return { meeting, error, setMeeting };
}

export function useMeetingSetup() {
  const [setup, setSetup] = useState<MeetingSetup>();
  const refresh = useCallback(() => api.getMeetingSetup().then(setSetup), []);
  useEffect(() => {
    refresh();
  }, [refresh]);
  // Cleanup being set up changes whether notes can be written.
  useTauriEvent("settings-changed", () => refresh());
  useTauriEvent("models-changed", () => refresh());
  return setup;
}
