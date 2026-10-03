// @pxl/web/shared/ui/toast/store.ts

import type { i32, u32 } from "@xpute/core/abi/word.ts";
import { create } from "zustand";

import { frontFirst } from "@/shared/ui/deck.ts";

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
  entries: Entry[];
  /** The entry at raw index `rotation` (mod count) is in front. */
  rotation: i32;
  dragging: boolean;
  pending: u32 | null;
}

export const useToastStore = create<ToastState>(() => ({ entries: [], rotation: 0, dragging: false, pending: null }));

const set = useToastStore.setState;
const get = useToastStore.getState;

// Before inserting at index 0 while turned, or the new entry lands behind the front card.
function unturned(): Entry[] {
  const { entries, rotation } = get();
  return frontFirst(entries, rotation);
}

function toFront(id: u32): void {
  const entries = unturned();
  const at = entries.findIndex((e) => e.id === id);
  if (at < 0) return;
  set({ entries: [entries[at], ...entries.slice(0, at), ...entries.slice(at + 1)], rotation: 0 });
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
  set({ entries: [{ id, ...spec }, ...unturned()], rotation: 0 });
}

export function toastDismiss(id: u32): void {
  const entries = unturned().filter((e) => e.id !== id);
  set({ entries, rotation: 0, pending: get().pending === id ? null : get().pending });
}

export function toastRotate(delta: i32): void {
  set({ rotation: get().rotation + delta });
}

export function setToastDragging(dragging: boolean): void {
  const { pending } = get();
  set({ dragging });
  if (!dragging && pending !== null) {
    set({ pending: null });
    toFront(pending);
  }
}
