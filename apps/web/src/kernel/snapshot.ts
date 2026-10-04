// @pxl/web/kernel/snapshot.ts

import { useSyncExternalStore } from "react";

import type { U8Array } from "@xpute/core/abi/array.ts";
import { TreeReader } from "@xpute/core/wire/xtp/mod.ts";
import { ActiveTool, DevicePoseStatus, MapStatus, read_snapshot, type Snapshot } from "@/kernel/abi/cmd.spec.ts";

const INITIAL: Snapshot = {
  map: { status: MapStatus.Idle, is_off_center: false, center: null },
  location: { status: DevicePoseStatus.Idle, has_location: false, error: null },
  companion: { person_ready: false, pet_ready: false },
  holding: { tool: ActiveTool.Pointer, channel: null, cube: 0, decor: 0, tier: 0 },
  orientation: { status: DevicePoseStatus.Idle, error: null },
  azimuth: 0,
  is_follow_mode: false,
  camera_busy: false,
  zooming: false,
  compass: CompassState.Hidden,
  settings: [],
  m_per_px: 0,
  place: [],
};

let snapshot: Snapshot = INITIAL;
const snapshot_listeners = new Set<() => void>();

/** Read whole now: the kernel writes the next one in the same place. */
export function set_snapshot(bytes: U8Array): void {
  snapshot = read_snapshot(new TreeReader(bytes).read_branch());
  for (const listener of snapshot_listeners) listener();
}

/** Calls `listener` with every snapshot the kernel posts from now on. */
function subscribe_snapshot(listener: () => void): () => void {
  snapshot_listeners.add(listener);
  return () => snapshot_listeners.delete(listener);
}

export function kernel_snapshot(): Snapshot {
  return snapshot;
}

/** The part of the kernel's snapshot a component shows. A component
 * re-renders only when what it selects changes, so `select` returns a
 * primitive or a reference the snapshot already holds: one that builds a
 * new array or object each call never compares equal and renders forever. */
export function useKernel<T>(select: (s: Snapshot) => T): T {
  return useSyncExternalStore(subscribe_snapshot, () => select(snapshot));
}
