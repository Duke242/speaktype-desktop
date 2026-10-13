import { useCallback, useEffect, useRef, useState } from "react";
import {
  api,
  type Meeting,
  type MeetingListItem,
  type MeetingSegment,
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
  const latest = useRef(0);
  const refresh = useCallback(() => {
    // A slower, older search mustn't replace a newer one's results.
    const request = ++latest.current;
    return api.listMeetings(query.trim()).then((found) => {
      if (request === latest.current) setItems(found);
    });
  }, [query]);
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
  // Segments that arrive while a fetch is out may be missing from its snapshot.
  const pending = useRef<MeetingSegment[] | null>(null);
  const latest = useRef(0);
  const refresh = useCallback(() => {
    const request = ++latest.current;
    pending.current ??= [];
    const done = () => {
      const missed = pending.current ?? [];
      pending.current = null;
      return missed;
    };
    return api
      .getMeeting(id)
      .then((m) => {
        if (request !== latest.current) return;
        setMeeting({ ...m, segments: done().reduce(insertSegment, m.segments) });
        setError(undefined);
      })
      .catch((e) => {
        if (request !== latest.current) return;
        done();
        setMeeting(null);
        setError(String(e));
      });
  }, [id]);
  useEffect(() => {
    refresh();
  }, [refresh]);
  useTauriEvent("meetings-changed", () => refresh());
  useTauriEvent<MeetingSegmentEvent>("meeting-segment", ({ payload }) => {
    if (payload.meetingId !== id) return;
    pending.current?.push(payload.segment);
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
