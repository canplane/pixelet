// @pxl/client/port/core.ts

/** The core as the client reaches it once it has started: its commands,
 * bound to the UI's link (`ui`), and the profiler's reads (`perf`), which run
 * where the core's memory is. They are set once, before anything uses them. */

import type { Link } from "@xpute/runtime";
import { ui_exports, type UiExports } from "@/abi/cmd.spec.ts";

export type Perf = (what: string, args: unknown[]) => Promise<unknown>;

export let core: UiExports;
export let ui: Link;
export let perf: Perf;

export function set_core(link: Link, read: Perf): void {
  core = ui_exports(link);
  ui = link;
  perf = read;
}
