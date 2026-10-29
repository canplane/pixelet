// @pxl/client/shell/world/toast/store.ts

import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import { create } from "zustand";

export const enum ToastTone {
  Loading = 0,
  Error = 1,
  Ready = 2,
}

export interface ToastSpec {
  message: string;
  tone: ToastTone;
}

interface Entry extends ToastSpec {
  /** The caller's numbering: a report shown again replaces itself, and of
   * one tone the lower outranks the higher. */
  id: u32;
}

/** Where a new card comes in: an error before a wait, a wait before news. */
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
  /** Front card first. */
  entries: Entry[];
  /** Turns so far, up positive: which way the cards last moved. */
  turns: i32;
  dragging: boolean;
  pending: u32 | null;
}

export const useToastStore = create<ToastState>(() => ({
  entries: [],
  turns: 0,
  dragging: false,
  pending: null,
}));

const set = useToastStore.setState;
const get = useToastStore.getState;

function toFront(id: u32): void {
  const { entries } = get();
  const at = entries.findIndex((e) => e.id === id);
  if (at < 0) return;
  set({
    entries: [entries[at], ...entries.slice(0, at), ...entries.slice(at + 1)],
  });
}

export function toastShow(id: u32, spec: ToastSpec): void {
  const { entries, dragging } = get();
  const shown = entries.find((e) => e.id === id);
  // Said again as it is, as a toast kept past its cause is: no card to bring forward.
  if (
    shown !== undefined &&
    shown.message === spec.message &&
    shown.tone === spec.tone
  )
    return;
  if (shown !== undefined) {
    set({ entries: entries.map((e) => (e.id === id ? { id, ...spec } : e)) });
    // Reshuffling mid-drag would move the held card out of front.
    if (dragging) set({ pending: id });
    else toFront(id);
    return;
  }
  // Over the front card if it outranks it, else before the first behind it that it outranks.
  const entry = { id, ...spec };
  const k = entries.findIndex((e) => outranks(entry, e));
  const at = k < 0 ? entries.length : k;
  set({ entries: [...entries.slice(0, at), entry, ...entries.slice(at)] });
}

export function toastDismiss(id: u32): void {
  const { entries, pending } = get();
  set({
    entries: entries.filter((e) => e.id !== id),
    pending: pending === id ? null : pending,
  });
}

/** Up (1) sends the front card to the back; down (-1) brings the back one forward. */
export function toastRotate(delta: i32): void {
  const { entries, turns } = get();
  const k = ((delta % entries.length) + entries.length) % entries.length;
  set({
    entries: [...entries.slice(k), ...entries.slice(0, k)],
    turns: turns + delta,
  });
}

export function setToastDragging(dragging: boolean): void {
  const { pending } = get();
  set({ dragging });
  if (!dragging && pending !== null) {
    set({ pending: null });
    toFront(pending);
  }
}
