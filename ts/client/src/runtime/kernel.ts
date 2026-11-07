// @pxl/client/runtime/kernel.ts

/** The kernel as the client reaches it once started: its commands, bound to
 * the shell's link (`ui`), and the profiler's reads (`perf`), made where the
 * kernel's memory is. Set once, before anything reaches them. */

import type { Link } from "@xpute/runtime";
import { shell_exports, type ShellExports } from "@/abi/cmd.spec.ts";

export type Perf = (what: string, args: unknown[]) => Promise<unknown>;

export let kernel: ShellExports;
export let ui: Link;
export let perf: Perf;

export function set_kernel(link: Link, read: Perf): void {
  kernel = shell_exports(link);
  ui = link;
  perf = read;
}
