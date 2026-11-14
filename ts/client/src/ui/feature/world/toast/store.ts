// @pxl/client/ui/feature/world/toast/store.ts

import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import { create } from "zustand";

import * as ToastTone from "./tone.ts";

type ToastTone = (typeof ToastTone)[keyof typeof ToastTone];
export { ToastTone };

export interface ToastSpec {
  message: string;
  tone: ToastTone;
}

interface Entry extends ToastSpec {
  /** The caller's number for the report. Showing the same number again
   * replaces its toast, and among toasts of one tone the lower number ranks
   * higher. */
  id: u32;
}

/** Decides where a new card comes in: an error goes before a wait, and a wait
 * before a ready word. */
const SEVERITY: Record<ToastTone, u32> = {
  [ToastTone.Error]: 0,
  [ToastTone.Loading]: 1,
  [ToastTone.Ready]: 2,
};

function outranks(a: Entry, b: Entry): boolean {
  return SEVERITY[a.tone] !== SEVERITY[b.tone]
    ? SEVERITY[a.tone] < SEVERITY[b.tone]
    : a.id < b.id;
}

interface ToastState {
  /** The cards, front card first. */
  entries: Entry[];
  /** The sum of the turns so far, a turn up counting as 1. The stack compares
   * it with its last value to tell which way the cards moved. */
  turn_cnt: i32;
  dragging: boolean;
  /** A toast that changed during a drag. It comes to the front when the drag
   * ends. */
  pending: u32 | null;
}

const store = create<ToastState>()(() => ({
  entries: [],
  turn_cnt: 0,
  dragging: false,
  pending: null,
}));

const set = store.setState;
const get = store.getState;

export function useToastEntries(): readonly Entry[] {
  return store((s) => s.entries);
}

export function useToastTurnCount(): i32 {
  return store((s) => s.turn_cnt);
}

export function toast_cnt(): u32 {
  return get().entries.length;
}

function to_front(id: u32): void {
  const { entries } = get();
  const at = entries.findIndex((e) => e.id === id);
  if (at < 0) return;
  set({
    entries: [entries[at], ...entries.slice(0, at), ...entries.slice(at + 1)],
  });
}

export function show_toast(id: u32, spec: ToastSpec): void {
  const { entries, dragging } = get();
  const shown = entries.find((e) => e.id === id);
  // The same toast shown again, as a toast held past its cause is, changes
  // nothing and does not come to the front.
  if (
    shown !== undefined &&
    shown.message === spec.message &&
    shown.tone === spec.tone
  )
    return;
  if (shown !== undefined) {
    set({ entries: entries.map((e) => (e.id === id ? { id, ...spec } : e)) });
    // Reordering during a drag would move the held card away from the front,
    // so the move waits for the drag to end.
    if (dragging) set({ pending: id });
    else to_front(id);
    return;
  }
  const entry = { id, ...spec };
  const k = entries.findIndex((e) => outranks(entry, e));
  const at = k < 0 ? entries.length : k;
  set({ entries: [...entries.slice(0, at), entry, ...entries.slice(at)] });
}

export function dismiss_toast(id: u32): void {
  const { entries, pending } = get();
  set({
    entries: entries.filter((e) => e.id !== id),
    pending: pending === id ? null : pending,
  });
}

/** A `delta` of 1, a swipe up, sends the front card to the back. A `delta` of
 * -1, a swipe down, brings the back card forward. */
export function rotate_toasts(delta: i32): void {
  const { entries, turn_cnt } = get();
  const k = ((delta % entries.length) + entries.length) % entries.length;
  set({
    entries: [...entries.slice(k), ...entries.slice(0, k)],
    turn_cnt: turn_cnt + delta,
  });
}

export function set_toast_dragging(dragging: boolean): void {
  const { pending } = get();
  set({ dragging });
  if (!dragging && pending !== null) {
    set({ pending: null });
    to_front(pending);
  }
}
