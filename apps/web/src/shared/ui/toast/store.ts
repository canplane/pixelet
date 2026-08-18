// @pxl/web/shared/ui/toast/store.ts

import type { ReactNode } from "react";

export type ToastRenderInfo = {
  index: number;
  stack_size: number;
  is_removing: boolean;
};
export type ToastRenderFn = (info: ToastRenderInfo) => ReactNode;

type Entry = { id: string; render: ToastRenderFn };

const REMOVE_GRACE_MS = 200;

let entries: Entry[] = [];
const removing_ids = new Set<string>();
const listeners = new Set<() => void>();

function notify(): void {
  for (const listener of listeners) listener();
}

// Must run before splicing anything in at raw index 0 (fresh insertion, or
// promote_to_front below) while a rotation offset is active — otherwise the
// new entry doesn't land at displayed depth 0, some other card does, and a
// user who just swiped card 3 to front sees a new toast appear behind it.
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

function promote_to_front(id: string): void {
  bake_in_rotation();
  const idx = entries.findIndex((e) => e.id === id);
  if (idx <= 0) return;
  entries = [entries[idx], ...entries.slice(0, idx), ...entries.slice(idx + 1)];
}

// Whether the user has a finger down on the front card (toast.tsx's
// swipe-to-rotate gesture). A background toast updating mid-gesture defers its
// promote_to_front instead of applying immediately: reshuffling depth 0 out
// from under an in-progress drag would leave the card being held no longer at
// depth 0. Applied once the drag actually ends instead.
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

  // A stale rotation from a previous stack survives silently once the list
  // empties (no mounted Toast is left to run its own reset effect) — without
  // this, the next arrival would compute its depth against that stale offset
  // instead of landing at depth 0.
  const was_empty = entries.length === 0;
  if (!was_empty) bake_in_rotation();

  entries = [{ id, render }, ...entries];
  removing_ids.delete(id);
  notify();
  if (was_empty) reset_toast_stack_rotation();
}

export function toast_dismiss(id: string): void {
  if (!entries.some((e) => e.id === id) || removing_ids.has(id)) return;
  removing_ids.add(id);
  notify();
  setTimeout(() => {
    // toast_custom() may have un-removed this id in the meantime (a fresh
    // update arriving inside the grace window) — without this check the timer
    // would still cut it, silently dropping a toast just told to stay.
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
