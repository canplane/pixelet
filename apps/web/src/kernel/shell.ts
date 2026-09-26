// @pxl/web/kernel/shell.ts

/**
 * What the page's UI reads of the kernel and what it asks of it
 * (crates/pxl/web/kernel/src/shell/shell.rs): the snapshot it renders
 * (snapshot.ts), and the commands its controls send, which the host hears
 * (platform/web/host.ts). A control's command is rung at once with no quota,
 * so what it asks of the host is heard inside the call.
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import { host_command, host_enable_sensors, host_tracking } from "@platform/host.ts";
import type { Command, Tracking } from "@/kernel/abi/cmd.spec.ts";

export { kernel_snapshot, useKernel } from "@/kernel/snapshot.ts";

/** Sends one of the UI's commands to the host. */
export function kernel_command(cmd: Command, pkt?: U8Array): void {
  host_command(cmd, pkt);
}

/** The tracking button: the host raises the compass's prompt where the
 * device gates it, and the kernel hears the answer before the press. */
export function kernel_tracking(tracking: Tracking): Promise<void> {
  return host_tracking(tracking);
}

/** The sensor button. */
export function kernel_enable_sensors(): void {
  host_enable_sensors();
}
