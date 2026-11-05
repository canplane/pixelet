// @pxl/client/port/snapshot.ts

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { Listeners } from "@xpute/kit/lang/listeners.ts";
import { TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
import {
  GroundStatus,
  read_snapshot,
  SensorStatus,
  type ShellSnapshotEvent,
  type Snapshot,
} from "@/abi/cmd.spec.ts";
import { host } from "@/host/kernel.ts";

const INIT: Snapshot = {
  pivot: {
    status: GroundStatus.Idle,
    is_off_center: false,
    lon_lat: null,
    outside: false,
  },
  location: { status: SensorStatus.Idle, has_location: false, err: null },
  companion: { person_ready: false, pet_ready: false },
  draft: null,
  orient: { status: SensorStatus.Idle, err: null },
  azimuth: 0,
  is_follow_mode: false,
  camera_busy: false,
  zooming: false,
  over_marker: false,
  settings: [],
  m_per_px: 0,
  division: [],
  previews: [],
  refused_files: [],
  stall: null,
  renderer_progress: 0,
  area_progress: 0,
};

let snapshot: Snapshot = INIT;
const listeners = new Listeners();

/** Read whole now: the kernel writes the next one in the same place. */
export function set_snapshot(bytes: U8Array): void {
  snapshot = read_snapshot(new TreeReader(bytes).read_branch());
  listeners.notify();
}

/** The kernel's SHELL_SNAPSHOT: too large for a ring frame, so it names where
 * the snapshot lies in the guest's memory, the host's until its head passes
 * the event, and read whole before then. */
export function snapshot_posted({ at, len }: ShellSnapshotEvent): void {
  set_snapshot(new Uint8Array(host.guest.mem, at, len));
}

export const watch_snapshot = listeners.watch;

export function current(): Snapshot {
  return snapshot;
}
