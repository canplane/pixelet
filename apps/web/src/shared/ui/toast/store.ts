// @pxl/web/shared/ui/toast/store.ts

import type { i32 } from "@xpute/core/abi/word.ts";
import { create } from "zustand";

/** What a toast says about the thing it reports on. */
export type ToastTone = "loading" | "error" | "ready";

export interface ToastSpec {
  message: string;
  tone: ToastTone;
}

interface Entry extends ToastSpec {
  id: string;
}

interface ToastState {
  /** Front first, before `rotation` is applied. */
  entries: Entry[];
  /** How far the deck has been turned by swiping: the entry at raw index
   * `rotation` (mod the count) is the one in front. */
  rotation: i32;
  /** A finger is on the front card; see `setDragging`. */
  dragging: boolean;
  /** An update that arrived mid-drag, brought to the front once it ends. */
  pending: string | null;
}

export const useToastStore = create<ToastState>(() => ({ entries: [], rotation: 0, dragging: false, pending: null }));

const set = useToastStore.setState;
const get = useToastStore.getState;

/** Where an entry at raw index `i` stands in the deck: 0 in front. */
export function toastDepth(i: number, rotation: i32, count: number): number {
  return count > 0 ? ((i - rotation) % count + count) % count : 0;
}

// Must run before anything goes in at raw index 0 (a fresh toast, or one
// brought to the front) while the deck is turned — otherwise the new entry
// does not land in front, some other card does, and a user who just swiped
// card 3 to the front sees a new toast appear behind it.
function unturned(): Entry[] {
  const { entries, rotation } = get();
  if (rotation === 0 || entries.length <= 1) return entries;
  return entries
    .map((e, i) => ({ e, depth: toastDepth(i, rotation, entries.length) }))
    .sort((a, b) => a.depth - b.depth)
    .map((x) => x.e);
}

function toFront(id: string): void {
  const entries = unturned();
  const at = entries.findIndex((e) => e.id === id);
  if (at < 0) return;
  set({ entries: [entries[at], ...entries.slice(0, at), ...entries.slice(at + 1)], rotation: 0 });
}

/** Shows `spec` under `id`, in front: a toast already up under that id is
 * replaced where it is and brought forward. */
export function toastShow(id: string, spec: ToastSpec): void {
  const { entries, dragging } = get();
  if (entries.some((e) => e.id === id)) {
    set({ entries: entries.map((e) => (e.id === id ? { id, ...spec } : e)) });
    // Reshuffling the front out from under a drag in progress would leave
    // the card being held no longer in front: it waits for the drag to end.
    if (dragging) set({ pending: id });
    else toFront(id);
    return;
  }
  set({ entries: [{ id, ...spec }, ...unturned()], rotation: 0 });
}

export function toastDismiss(id: string): void {
  const entries = unturned().filter((e) => e.id !== id);
  set({ entries, rotation: 0, pending: get().pending === id ? null : get().pending });
}

/** Turns the deck a card forward (1) or back (−1). */
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
