// @pxl/client/service/core/core.ts

/** The core as the client reaches it once it has started: its commands,
 * bound to the UI's link (`ui`), and the profiler's reads (`perf`), which run
 * where the core's memory is. They are set once, before anything uses them. */

import type { Link } from "@xpute/runtime";
import type { UiProtocol } from "@/abi/cmd.spec.ts";

export type Perf = (what: string, args: unknown[]) => Promise<unknown>;

export let core: UiProtocol;
export let ui: Link;
export let perf: Perf;

export function set_core(link: Link, read: Perf): void {
  core = link;
  ui = link;
  perf = read;
}
