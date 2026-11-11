// @pxl/client/service/core/snapshot.ts

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { ListenerSet } from "@xpute/kit/lang/listener.ts";
import { TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
import {
  GroundStatus,
  read_snapshot,
  SensorStatus,
  type UiView,
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
const listeners = new ListenerSet();

/** `bytes` are the snapshot as the core named it when it sent it, valid only
 * inside the call that hands them over, so the snapshot is read whole at
 * once. */
export function set_snapshot(bytes: U8Array): void {
  snapshot = read_snapshot(new TreeReader(bytes).read_branch());
  listeners.notify();
}

export const watch_snapshot = listeners.watch;

export function cur(): Snapshot {
  return snapshot;
}

let view: UiView = { azimuth: 0, m_per_px: 0 };
const view_listeners = new ListenerSet();

/** Takes the core's UI_VIEW, which a rotation or a zoom posts every frame.
 * Only what watches the view is told, so the snapshot's readers are not woken
 * every frame. */
export function view_posted(e: UiView): void {
  view = e;
  view_listeners.notify();
}

export const watch_view = view_listeners.watch;

export function cur_view(): UiView {
  return view;
}
