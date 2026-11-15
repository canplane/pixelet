// @pxl/client/service/core/overlay.ts

import type { Remote } from "@xpute/link";
import { ListenerSet } from "@xpute/kit/lang/listener.ts";
import { mount, type Snapshot } from "@/abi/overlay.ts";
import { GROUND_STATUS_IDLE } from "@/abi/overlay.enum.ts";

const INIT: Snapshot = {
  pivot: {
    status: GROUND_STATUS_IDLE,
    is_off_center: false,
    lon_lat: null,
    outside: false,
  },
  companion: { person_ready: false, pet_ready: false },
  draft: null,
  is_follow_mode: false,
  camera_busy: false,
  zooming: false,
  over_marker: false,
  settings: new Uint32Array(),
  division: [],
  character_surfaces: new Uint32Array(),
  stall: null,
  renderer_progress: 0,
  area_progress: 0,
};

let snapshot: Snapshot = INIT;
const listeners = new ListenerSet();

/** The core's snapshot, replaced whole when it changes. */
function set_snapshot(s: Snapshot): void {
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
function view_posted(e: UiView): void {
  view = e;
  view_listeners.notify();
}

export const watch_view = view_listeners.watch;

export function cur_view(): UiView {
  return view;
}

/** Serves `overlay` on `remote`, the core's snapshot and view, before the
 * core boots. */
export function serve_overlay(remote: Remote): void {
  mount(remote, {
    show: set_snapshot,
    view: (azimuth, m_per_px) => view_posted({ azimuth, m_per_px }),
  });
}
