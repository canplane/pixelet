// @pxl/web/kit/ui/toast/store.ts

import type { ReactNode } from "react";

export type ToastRenderInfo = {
  index: number;
  stackSize: number;
  isRemoving: boolean;
};
export type ToastRenderFn = (info: ToastRenderInfo) => ReactNode;

type Entry = { id: string; render: ToastRenderFn };

const REMOVE_GRACE_MS = 200;

let entries: Entry[] = [];
const removingIds = new Set<string>();
const listeners = new Set<() => void>();

function notify(): void {
  for (const listener of listeners) listener();
}

// Must run before splicing anything in at raw index 0 (fresh insertion, or
// promoteToFront below) while a rotation offset is active — otherwise the
// new entry doesn't land at displayed depth 0, some other card does, and a
// user who just swiped card 3 to front sees a new toast appear behind it.
function bakeInRotation(): void {
  if (rotation === 0 || entries.length <= 1) return;
  const n = entries.length;
  const r = rotation;
  entries = entries
    .map((e, i) => ({ e, depth: ((i - r) % n + n) % n }))
    .sort((a, b) => a.depth - b.depth)
    .map((x) => x.e);
  resetToastStackRotation();
}

function promoteToFront(id: string): void {
  bakeInRotation();
  const idx = entries.findIndex((e) => e.id === id);
  if (idx <= 0) return;
  entries = [entries[idx], ...entries.slice(0, idx), ...entries.slice(idx + 1)];
}

// Whether the user has a finger down on the front card (toast.tsx's
// swipe-to-rotate gesture). A background toast updating mid-gesture defers its
// promoteToFront instead of applying immediately: reshuffling depth 0 out
// from under an in-progress drag would leave the card being held no longer at
// depth 0. Applied once the drag actually ends instead.
let isDragging = false;
let pendingPromoteId: string | null = null;

export function setToastDragActive(active: boolean): void {
  isDragging = active;
  if (!active && pendingPromoteId !== null) {
    const id = pendingPromoteId;
    pendingPromoteId = null;
    promoteToFront(id);
    notify();
  }
}

export function toastCustom(id: string, render: ToastRenderFn): void {
  const existingPos = entries.findIndex((e) => e.id === id);
  if (existingPos !== -1) {
    entries = entries.map((e, i) => (i === existingPos ? { id, render } : e));
    removingIds.delete(id);

    if (isDragging) {
      pendingPromoteId = id;
    } else {
      promoteToFront(id);
    }

    notify();
    return;
  }

  // A stale rotation from a previous stack survives silently once the list
  // empties (no mounted Toast is left to run its own reset effect) — without
  // this, the next arrival would compute its depth against that stale offset
  // instead of landing at depth 0.
  const wasEmpty = entries.length === 0;
  if (!wasEmpty) bakeInRotation();

  entries = [{ id, render }, ...entries];
  removingIds.delete(id);
  notify();
  if (wasEmpty) resetToastStackRotation();
}

export function toastDismiss(id: string): void {
  if (!entries.some((e) => e.id === id) || removingIds.has(id)) return;
  removingIds.add(id);
  notify();
  setTimeout(() => {
    // toastCustom() may have un-removed this id in the meantime (a fresh
    // update arriving inside the grace window) — without this check the timer
    // would still cut it, silently dropping a toast just told to stay.
    if (!removingIds.has(id)) return;
    entries = entries.filter((e) => e.id !== id);
    removingIds.delete(id);
    notify();
  }, REMOVE_GRACE_MS);
}

export function getToastEntries(): Entry[] {
  return entries;
}

export function isToastRemoving(id: string): boolean {
  return removingIds.has(id);
}

export function subscribeToastEntries(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

type RotationListener = (rotation: number, direction: -1 | 0 | 1) => void;

let rotation = 0;
const rotationListeners = new Set<RotationListener>();

function notify_rotation(direction: -1 | 0 | 1): void {
  for (const listener of rotationListeners) listener(rotation, direction);
}

export function getToastStackRotation(): number {
  return rotation;
}

export function rotateToastStack(delta: number): void {
  rotation += delta;
  notify_rotation(delta < 0 ? -1 : delta > 0 ? 1 : 0);
}

export function resetToastStackRotation(): void {
  if (rotation === 0) return;
  rotation = 0;
  notify_rotation(0);
}

export function subscribeToastStackRotation(listener: RotationListener): () => void {
  rotationListeners.add(listener);
  return () => {
    rotationListeners.delete(listener);
  };
}
