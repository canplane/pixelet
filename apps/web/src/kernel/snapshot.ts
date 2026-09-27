// @pxl/web/kernel/snapshot.ts

/**
 * The one snapshot the UI renders (crates/pxl/web/kernel/src/shell/shell.rs):
 * a Snapshot packet in a block the kernel holds, read whole when it is
 * signaled and replaced whole — a component subscribes and re-renders on the
 * part it selects. Its own module so the host that fills it and the shell
 * that reads it do not reach through each other.
 */

import { useSyncExternalStore } from "react";

import type { U8Array } from "@xpute/core/abi/array.ts";
import { TreeReader } from "@xpute/core/wire/xtp/mod.ts";
import { CompassState, DevicePoseStatus, MapStatus, read_snapshot, type Snapshot } from "@/kernel/abi/cmd.spec.ts";

const INITIAL: Snapshot = {
  map: { status: MapStatus.Idle, is_off_center: false, center: null, page: null },
  location: { status: DevicePoseStatus.Idle, has_location: false, error: null },
  companion: { person_ready: false, pet_ready: false },
  orientation: { status: DevicePoseStatus.Idle, heading: null },
  azimuth: 0,
  is_follow_mode: false,
  camera_busy: false,
  zooming: false,
  compass: CompassState.Hidden,
  settings: [],
  scale_bar: { total_meters: 0, bar_width_px: 0 },
  place: [],
  object_sizes: [],
};

let snapshot: Snapshot = INITIAL;
const snapshot_listeners = new Set<() => void>();

/** Reads the kernel's snapshot out of the bytes it holds it in: all of it,
 * now, since the kernel writes the next one there. */
export function set_snapshot(bytes: U8Array): void {
  snapshot = read_snapshot(new TreeReader(bytes).read_branch());
  for (const listener of snapshot_listeners) listener();
}

/** Calls `listener` with every snapshot the kernel posts from now on. */
export function subscribe_snapshot(listener: () => void): () => void {
  snapshot_listeners.add(listener);
  return () => snapshot_listeners.delete(listener);
}

export function kernel_snapshot(): Snapshot {
  return snapshot;
}

/** The part of the kernel's snapshot a component shows. */
export function useKernel<T>(select: (s: Snapshot) => T): T {
  return select(useSyncExternalStore(subscribe_snapshot, kernel_snapshot));
}
