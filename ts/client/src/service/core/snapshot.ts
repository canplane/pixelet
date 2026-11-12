// @pxl/client/service/core/snapshot.ts

import { ListenerSet } from "@xpute/kit/lang/listener.ts";
import { GroundStatus, SensorStatus, type Snapshot } from "@/abi/overlay.ts";

const INIT: Snapshot = {
  pivot: {
    status: GroundStatus.Idle,
    is_off_center: false,
    lon_lat: null,
    outside: false,
  },
  location: {
    status: SensorStatus.Idle,
    has_location: false,
    watching: false,
    err: null,
  },
  companion: { person_ready: false, pet_ready: false },
  draft: null,
  orient: { status: SensorStatus.Idle },
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

/** The core's snapshot, replaced whole when it changes. */
export function set_snapshot(s: Snapshot): void {
  snapshot = s;
  listeners.notify();
}

export const watch_snapshot = listeners.watch;

export function cur(): Snapshot {
  return snapshot;
}

/** The view's azimuth, and the meters a logical pixel spans at the pivot. */
export interface UiView {
  azimuth: number;
  m_per_px: number;
}

let view: UiView = { azimuth: 0, m_per_px: 0 };
const view_listeners = new ListenerSet();

/** Takes the core's view, which a rotation or a zoom posts every frame.
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
