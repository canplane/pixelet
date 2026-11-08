// @xpute/runtime/start.native.ts

/**
 * Starts a device's core on its own thread beside the UI. The runtime runs on
 * the UI's JavaScript thread and is itself the UI's link.
 */

import { boot, type Link, type Manifest, type Ui } from "./runtime.ts";
import type { Grant } from "./sched/policy.ts";

export function start<G extends Grant>(m: Manifest<G>, ui: Ui): Promise<Link> {
  return boot(m, ui);
}
