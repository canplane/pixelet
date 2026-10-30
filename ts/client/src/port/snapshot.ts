// @pxl/client/port/snapshot.ts

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { Listeners } from "@xpute/kit/lang/listeners.ts";
import { type BranchCursor, TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
import {
  ActiveTool,
  GroundStatus,
  read_snapshot,
  read_world_snapshot,
  SensorStatus,
  type Snapshot,
} from "@/abi/cmd.spec.ts";
import { HOST } from "@/host/mount.ts";

const INIT: Snapshot = {
  pivot: {
    status: GroundStatus.Idle,
    is_off_center: false,
    lon_lat: null,
    outside: false,
  },
  location: { status: SensorStatus.Idle, has_location: false, err: null },
  companion: { person_ready: false, pet_ready: false },
  holding: {
    tool: ActiveTool.Pointer,
    cube: 0,
    decor: 0,
    tier: 0,
    marker: false,
    words: "",
  },
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
  kept_refused: [],
  stall: null,
};

let snapshot: Snapshot = INIT;
const listeners = new Listeners();

/** Read whole now: the kernel writes the next one in the same place. */
export function set_snapshot(bytes: U8Array): void {
  snapshot = read_snapshot(new TreeReader(bytes).read_branch());
  listeners.notify();
}

/** The kernel's WORLD_SNAPSHOT: too large for a ring frame, so it names where
 * the snapshot lies in the guest's memory, valid until the next, a turn away. */
export function snapshot_posted(f: BranchCursor): void {
  const { at, len } = read_world_snapshot(f);
  set_snapshot(new Uint8Array(HOST.guest.mem, at, len));
}

export const watch_snapshot = listeners.watch;

export function kernel_snapshot(): Snapshot {
  return snapshot;
}
