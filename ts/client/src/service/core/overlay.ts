// @pxl/client/service/core/overlay.ts

import type { Callee } from "@xpute/link/link.ts";
import { ListenerSet } from "@xpute/kit/lang/listener.ts";
import { mount, type Snapshot } from "@/abi/overlay.ts";

const INIT: Snapshot = {
  pivot: {
    ground_shown: false,
    is_off_center: false,
    lon_lat: null,
    outside: false,
  },
  companion_ready: false,
  drafting: false,
  is_follow_mode: false,
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
function setSnapshot(s: Snapshot): void {
  snapshot = s;
  listeners.notify();
}

export const watchSnapshot = listeners.watch;

export function currentSnapshot(): Snapshot {
  return snapshot;
}

/** The view's azimuth, and the meters a logical pixel spans at the pivot. */
export interface UiView {
  azimuth: number;
  mPerPx: number;
}

let view: UiView = { azimuth: 0, mPerPx: 0 };
const viewListeners = new ListenerSet();

/** Takes the core's view, which a rotation or a zoom posts every frame.
 * Only what watches the view is told, so the snapshot's readers are not woken
 * every frame. */
function viewPosted(e: UiView): void {
  view = e;
  viewListeners.notify();
}

export const watchView = viewListeners.watch;

export function currentView(): UiView {
  return view;
}

let draft = "";

/** The text of the marker's draft as the core told it, as the draft opened
 * or as this UI attached to it (pxl:core overlay `draft`). */
export function currentDraft(): string {
  return draft;
}

/** Mounts `overlay` on `ui`, the UI's end, before the core boots: the
 * core's snapshot and view, each answered once it is taken. */
export function mountOverlay(ui: Callee): void {
  mount(ui, {
    show: async (snapshot) => setSnapshot(snapshot),
    draft: async (text) => {
      draft = text;
    },
    view: async (azimuth, mPerPx) => viewPosted({ azimuth, mPerPx }),
  });
}
