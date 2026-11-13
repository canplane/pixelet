// @pxl/client/service/core/core.ts

/** The core as the UI reaches it: the UI's remote end of the link (`ui`),
 * which the core's interfaces are called over by their named handles, and the
 * interfaces the UI serves, mounted for the core before it boots. */

import type { Remote } from "@xpute/link";
import * as overlay from "@/abi/overlay.ts";
import { set_snapshot, view_posted } from "@/service/core/snapshot.ts";

export let ui: Remote;

/** Mounts the interfaces the UI serves on `remote`, before the core boots. */
export function serve_overlay(remote: Remote): void {
  ui = remote;
  overlay.mount(remote, {
    show: set_snapshot,
    view: (azimuth, m_per_px) => view_posted({ azimuth, m_per_px }),
  });
}

