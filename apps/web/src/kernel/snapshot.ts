// @pxl/web/kernel/snapshot.ts

import { useSyncExternalStore } from "react";

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
import { ActiveTool, DevicePoseStatus, GroundStatus, read_snapshot, type Snapshot } from "@/kernel/abi/cmd.spec.ts";

const INITIAL: Snapshot = {
  focus: { status: GroundStatus.Idle, is_off_center: false, center: null, outside: false },
  location: { status: DevicePoseStatus.Idle, has_location: false, error: null },
  companion: { person_ready: false, pet_ready: false },
  holding: { tool: ActiveTool.Pointer, cube: 0, decor: 0, tier: 0, marker: false, words: "" },
  azimuth: 0,
  camera_busy: false,
  zooming: false,
  settings: [],
  m_per_px: 0,
  division: [],
  previews: [],
  stall: null,
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
