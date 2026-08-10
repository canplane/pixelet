// @pxl/web/shared/ui/toast/toast_store.ts
// Owns the toast list directly (no Sonner or similar library) so the
// card-deck + swipe-to-rotate feature built on top of it fully controls
// layout, animation, and gesture — see toast.tsx's own doc comment for why
// that stopped being negotiable with a library that assumes it owns those.

import type { ReactNode } from "react";

/** index 0 is always the current front card; stack_size is the same number for every entry (total count), passed through so toast.tsx never has to re-derive it. */
export type ToastRenderInfo = {
  index: number;
  stack_size: number;
  is_removing: boolean;
};
export type ToastRenderFn = (info: ToastRenderInfo) => ReactNode;

type Entry = { id: string; render: ToastRenderFn };

/** How long a dismissed toast stays mounted (as is_removing:true) before actually being dropped from the list — gives toast.tsx's own fade-out transition time to finish. */
const REMOVE_GRACE_MS = 200;

let entries: Entry[] = [];
const removing_ids = new Set<string>();
const listeners = new Set<() => void>();

function notify(): void {
  for (const listener of listeners) listener();
}

/**
 * Physically reorders `entries` to match their currently-*displayed*
 * (rotated) order, and folds `rotation` back to 0. Any operation that's
 * about to move an entry to raw index 0 (a fresh insertion, or promoting an
 * updated background toast — see promote_to_front below) needs this done
 * first: rotated_depth's own modulus is entries.length, so splicing raw
 * index 0 in while a stale rotation offset is still active makes some
 * unrelated toast land at depth 0 instead of the one actually arriving —
 * reported as "I swiped card 3 to the front, then a new toast showed up
 * somewhere that wasn't in front of 3." No-op if there's nothing to fold.
 */
function bake_in_rotation(): void {
  if (rotation === 0 || entries.length <= 1) return;
  const n = entries.length;
  const r = rotation;
  entries = entries
    .map((e, i) => ({ e, depth: ((i - r) % n + n) % n }))
    .sort((a, b) => a.depth - b.depth)
    .map((x) => x.e);
  reset_toast_stack_rotation();
}

/** Moves an already-present toast to the front of the deck — used when a
 * background (currently non-front) toast's content changes, so the user
 * doesn't have to swipe back to it to notice. See set_toast_drag_active
 * below for why this can't always happen immediately. */
function promote_to_front(id: string): void {
  bake_in_rotation();
  const idx = entries.findIndex((e) => e.id === id);
  if (idx <= 0) return; // already front, or not found
  entries = [entries[idx], ...entries.slice(0, idx), ...entries.slice(idx + 1)];
}

/**
 * Whether the user currently has a finger/pointer down on the front card
 * (toast.tsx's own swipe-to-rotate gesture — see handle_pointer_down and
 * finish_drag there). A background toast updating *during* that gesture
 * defers its own promote_to_front: reshuffling depth 0 out from under an
 * in-progress drag would leave the card the user is physically holding no
 * longer being the one at depth 0, an inconsistency nothing here is built
 * to handle mid-gesture. Applied the moment the drag actually ends instead.
 */
let is_dragging = false;
let pending_promote_id: string | null = null;

export function set_toast_drag_active(active: boolean): void {
  is_dragging = active;
  if (!active && pending_promote_id !== null) {
    const id = pending_promote_id;
    pending_promote_id = null;
    promote_to_front(id);
    notify();
  }
}

/** Creates a new toast, or — if `id` already exists — updates its render
 * function in place and brings it to the front of the deck (immediately, or
 * once any in-progress drag ends — see set_toast_drag_active) so a change
 * to a currently-hidden toast doesn't go unnoticed behind the front card. */
export function toast_custom(id: string, render: ToastRenderFn): void {
  const existing_pos = entries.findIndex((e) => e.id === id);
  if (existing_pos !== -1) {
    entries = entries.map((e, i) => (i === existing_pos ? { id, render } : e));
    removing_ids.delete(id);

    if (is_dragging) {
      pending_promote_id = id;
    } else {
      promote_to_front(id);
    }

    notify();
    return;
  }

  // A stale rotation left over from a previous stack (e.g. everyone swiped
  // through, then all got dismissed) would otherwise survive silently —
  // the list dropping to 0 means no Toast instance exists to run its own
  // stack_size<=1 reset effect, so nothing ever clears it. The next toast
  // to arrive would then compute its depth against that stale offset
  // instead of landing at depth 0 like a brand new arrival always should.
  const was_empty = entries.length === 0;
  if (!was_empty) bake_in_rotation();

  entries = [{ id, render }, ...entries];
  removing_ids.delete(id);
  notify();
  if (was_empty) reset_toast_stack_rotation();
}

/** Marks a toast as removing (triggers its own fade-out render), then actually drops it from the list after REMOVE_GRACE_MS. */
export function toast_dismiss(id: string): void {
  if (!entries.some((e) => e.id === id) || removing_ids.has(id)) return;
  removing_ids.add(id);
  notify();
  setTimeout(() => {
    // toast_custom() may have un-removed this id in the meantime (a fresh
    // update for the same id arriving inside the grace window) — without
    // this check, this timer would still cut it from `entries` regardless,
    // silently dropping a toast that had just been told to stay.
    if (!removing_ids.has(id)) return;
    entries = entries.filter((e) => e.id !== id);
    removing_ids.delete(id);
    notify();
  }, REMOVE_GRACE_MS);
}

export function get_toast_entries(): Entry[] {
  return entries;
}

export function is_toast_removing(id: string): boolean {
  return removing_ids.has(id);
}

export function subscribe_toast_entries(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/**
 * Card-deck rotation offset — a swipe on the front toast rotates the whole
 * deck (front card moves to the back, the next one takes its place,
 * circular-queue style). Shared across every stacked toast: toaster.tsx
 * mounts each entry as its own independent <Toast> render tree (not
 * siblings sharing a parent's state), so a useState in toast.tsx couldn't
 * be seen by the other toast instances that also need to react to the same
 * swipe. Plain module-level state + a listener Set, same shape as the
 * entries pub/sub above, kept in this file (not a separate store) since
 * it's only ever meaningful together with the entries it indexes into —
 * splitting it out previously just meant this reset-on-empty logic had to
 * reach back across a module boundary via its own subscription instead of
 * the plain function call it is now.
 *
 * Deliberately just an offset, not "which toast id is front" — toast.tsx
 * combines this with its own `index` (0 is always the newest toast) to
 * derive each toast's displayed depth.
 */
// direction accompanies each notification (not just the new rotation
// number) so toast.tsx can tell *how* a toast arrived at depth 0 — the
// card that wraps around from the very back to the front looks
// structurally identical whether a swipe-up stepped the next card forward
// or a swipe-down pulled the back card around, but a downward swipe wants
// that arrival to visibly descend into place instead of just interpolating
// its stack position like every other depth change already does.
type RotationListener = (rotation: number, direction: -1 | 0 | 1) => void;

let rotation = 0;
const rotation_listeners = new Set<RotationListener>();

function notify_rotation(direction: -1 | 0 | 1): void {
  for (const listener of rotation_listeners) listener(rotation, direction);
}

export function get_toast_stack_rotation(): number {
  return rotation;
}

export function rotate_toast_stack(delta: number): void {
  rotation += delta;
  notify_rotation(delta < 0 ? -1 : delta > 0 ? 1 : 0);
}

export function reset_toast_stack_rotation(): void {
  if (rotation === 0) return;
  rotation = 0;
  notify_rotation(0);
}

export function subscribe_toast_stack_rotation(listener: RotationListener): () => void {
  rotation_listeners.add(listener);
  return () => {
    rotation_listeners.delete(listener);
  };
}
