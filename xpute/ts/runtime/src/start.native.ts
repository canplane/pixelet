// @xpute/runtime/start.native.ts

/**
 * A device's core, on its own thread beside the UI: the runtime, on the UI's
 * JavaScript thread, is the UI's link.
 */

import { boot, type Link, type Manifest, type Ui } from "./runtime.ts";
import type { Grant } from "./sched/policy.ts";

/** The core of `m` booted beside the UI. */
export function start<G extends Grant>(m: Manifest<G>, ui: Ui): Promise<Link> {
  return boot(m, ui);
}
