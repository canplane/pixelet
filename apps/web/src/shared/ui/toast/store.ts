// @pxl/web/shared/ui/toast/store.ts

import type { i32, u32 } from "@xpute/core/abi/word.ts";
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
  /** The caller's numbering: a report shown again replaces itself. */
  id: u32;
}

interface ToastState {
  /** Front card first. */
  entries: Entry[];
  /** Turns so far, up positive: which way the cards last moved. */
  turns: i32;
  dragging: boolean;
  pending: u32 | null;
}

export const useToastStore = create<ToastState>(() => ({ entries: [], turns: 0, dragging: false, pending: null }));

const set = useToastStore.setState;
const get = useToastStore.getState;

function toFront(id: u32): void {
  const { entries } = get();
  const at = entries.findIndex((e) => e.id === id);
  if (at < 0) return;
  set({ entries: [entries[at], ...entries.slice(0, at), ...entries.slice(at + 1)] });
}

export function toastShow(id: u32, spec: ToastSpec): void {
  const { entries, dragging } = get();
  if (entries.some((e) => e.id === id)) {
    set({ entries: entries.map((e) => (e.id === id ? { id, ...spec } : e)) });
    // Reshuffling mid-drag would move the held card out of front.
    if (dragging) set({ pending: id });
    else toFront(id);
    return;
  }
  set({ entries: [{ id, ...spec }, ...entries] });
}

export function toastDismiss(id: u32): void {
  const { entries, pending } = get();
  set({ entries: entries.filter((e) => e.id !== id), pending: pending === id ? null : pending });
}

/** Up (1) sends the front card to the back; down (-1) brings the back one forward. */
export function toastRotate(delta: i32): void {
  const { entries, turns } = get();
  const k = ((delta % entries.length) + entries.length) % entries.length;
  set({ entries: [...entries.slice(k), ...entries.slice(0, k)], turns: turns + delta });
}

export function setToastDragging(dragging: boolean): void {
  const { pending } = get();
  set({ dragging });
  if (!dragging && pending !== null) {
    set({ pending: null });
    toFront(pending);
  }
}
