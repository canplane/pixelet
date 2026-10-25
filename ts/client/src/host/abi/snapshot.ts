// @pxl/client/host/abi/snapshot.ts

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { Listeners } from "@xpute/kit/lang/listeners.ts";
import { TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
import { ActiveTool, DevicePoseStatus, GroundStatus, read_snapshot, type Snapshot } from "@/host/abi/cmd.spec.ts";

const INITIAL: Snapshot = {
  focus: { status: GroundStatus.Idle, is_off_center: false, center: null, outside: false },
  location: { status: DevicePoseStatus.Idle, has_location: false, error: null },
  companion: { person_ready: false, pet_ready: false },
  holding: { tool: ActiveTool.Pointer, cube: 0, decor: 0, tier: 0, marker: false, words: "" },
  orientation: { status: DevicePoseStatus.Idle, error: null },
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

let snapshot: Snapshot = INITIAL;
const listeners = new Listeners();

/** Read whole now: the kernel writes the next one in the same place. */
export function set_snapshot(bytes: U8Array): void {
  snapshot = read_snapshot(new TreeReader(bytes).read_branch());
  listeners.notify();
}

export const watch_snapshot = listeners.watch;

export function kernel_snapshot(): Snapshot {
  return snapshot;
}
