// @pxl/client/port/snapshot.ts

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { Listeners } from "@xpute/kit/lang/listeners.ts";
import { TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
import {
  GroundStatus,
  read_snapshot,
  SensorStatus,
  type ShellView,
  type Snapshot,
} from "@/abi/cmd.spec.ts";

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
  is_follow_mode: false,
  camera_busy: false,
  zooming: false,
  over_marker: false,
  settings: [],
  division: [],
  character_surfaces: [],
  refused_files: [],
  stall: null,
  renderer_progress: 0,
  area_progress: 0,
};

let snapshot: Snapshot = INIT;
const listeners = new Listeners();

/** The kernel's SHELL_SNAPSHOT, its bytes where it lies, valid until the
 * runtime's head passes it: read whole now, as the kernel writes the next in
 * the same place. */
export function set_snapshot(bytes: U8Array): void {
  snapshot = read_snapshot(new TreeReader(bytes).read_branch());
  listeners.notify();
}

export const watch_snapshot = listeners.watch;

export function current(): Snapshot {
  return snapshot;
}

let view: ShellView = { azimuth: 0, m_per_px: 0 };
const view_listeners = new Listeners();

/** The kernel's SHELL_VIEW, which a rotation or a zoom posts every frame:
 * told only to what watches the view, the snapshot's readers left asleep. */
export function view_posted(e: ShellView): void {
  view = e;
  view_listeners.notify();
}

export const watch_view = view_listeners.watch;

export function current_view(): ShellView {
  return view;
}
