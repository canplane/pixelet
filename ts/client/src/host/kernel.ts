// @pxl/client/host/kernel.ts

/** The kernel as the client reaches it once booted (abi/boot.ts): the host
 * xpute booted it on, the shell port's commands bound to that host, and the
 * pacing its turns run by, which an input makes interactive. Set once, before
 * anything reaches them. */

import type { FramePacing, Host } from "@xpute/host";
import { shell_exports, type ShellExports } from "@/abi/cmd.spec.ts";

export let host: Host;
export let kernel: ShellExports;
export let pacing: FramePacing;

export function set_host(h: Host, p: FramePacing): void {
  host = h;
  kernel = shell_exports(h);
  pacing = p;
}
