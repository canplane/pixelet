// @pxl/web/kernel/shell.ts

/**
 * What the page's UI reads of the kernel and what it asks of it
 * (crates/pxl/web/kernel/src/shell/shell.rs): one snapshot it renders, and
 * the commands its controls send.
 *
 * The snapshot arrives as a Snapshot packet in a block the kernel holds, read
 * whole when it is signaled and replaced whole — a component subscribes and
 * re-renders on the part it selects. A control's command is rung at once with
 * no quota, so what it asks of the host is heard inside the call.
 */

import { useSyncExternalStore } from "react";

import type { U8Array } from "@xpute/core/abi/array.ts";
import { TreeReader } from "@xpute/core/wire/xtp/mod.ts";
import { host_command, host_enable_sensors, host_tracking } from "@platform/host.ts";
import { type Command, DevicePoseStatus, MapStatus, read_snapshot, type Snapshot, type Tracking } from "@/kernel/abi/cmd.spec.ts";

const INITIAL: Snapshot = {
  map: { status: MapStatus.Idle, is_off_center: false, center: null, page: null },
  location: { status: DevicePoseStatus.Idle, has_location: false, error: null },
  companion: { person_ready: false, pet_ready: false },
  orientation: { status: DevicePoseStatus.Idle, heading: null },
  azimuth: 0,
  is_follow_mode: false,
  camera_busy: false,
  show_compass: false,
  settings: [],
  scale_bar: { total_meters: 0, bar_width_px: 0, is_default_zoom: true },
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

/** Sends one of the UI's commands to the host (platform/web/host.ts). */
export function kernel_command(cmd: Command, pkt?: U8Array): void {
  host_command(cmd, pkt);
}

/** The tracking button: the host raises the compass's prompt where the
 * device gates it, and the kernel hears the answer before the press. */
export function kernel_tracking(tracking: Tracking): Promise<void> {
  return host_tracking(tracking);
}

/** The sensor button. */
export function kernel_enable_sensors(): void {
  host_enable_sensors();
}
