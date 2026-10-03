// @pxl/web/kernel/snapshot.ts

import { useSyncExternalStore } from "react";

import type { U8Array } from "@xpute/core/abi/array.ts";
import { TreeReader } from "@xpute/core/wire/xtp/mod.ts";
import { DevicePoseStatus, MapStatus, read_snapshot, type Snapshot } from "@/kernel/abi/cmd.spec.ts";

const INITIAL: Snapshot = {
  map: { status: MapStatus.Idle, is_off_center: false, center: null },
  location: { status: DevicePoseStatus.Idle, has_location: false, error: null },
  companion: { person_ready: false, pet_ready: false },
  orientation: { status: DevicePoseStatus.Idle },
  azimuth: 0,
  is_follow_mode: false,
  camera_busy: false,
  zooming: false,
  settings: [],
  m_per_px: 0,
  place: [],
  previews: [],
};

let snapshot: Snapshot = INITIAL;
const snapshot_listeners = new Set<() => void>();

/** Read whole now: the kernel writes the next one in the same place. */
export function set_snapshot(bytes: U8Array): void {
  snapshot = read_snapshot(new TreeReader(bytes).read_branch());
  for (const listener of snapshot_listeners) listener();
}

function subscribe_snapshot(listener: () => void): () => void {
  snapshot_listeners.add(listener);
  return () => snapshot_listeners.delete(listener);
}

export function kernel_snapshot(): Snapshot {
  return snapshot;
}

/** `select` must return a primitive or an existing reference; a fresh object renders forever. */
export function useKernel<T>(select: (s: Snapshot) => T): T {
  return useSyncExternalStore(subscribe_snapshot, () => select(snapshot));
}
