// @pxl/native/platform/host.ts

/**
 * The kernel's host on a device: the Rust core linked into the app
 * (platform/core.ts), rung from the app's frame by the same scheduler and
 * rings the page uses (kernel/). The shell asks it what it asks any host — to
 * start, to hear a control's command, to know what the pointer holds — and
 * nothing of how. What a device does not have yet is left out rather than
 * faked: no compass prompt, no sensors' watches.
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import type { u32 } from "@xpute/core/abi/word.ts";
import type { GroundChannelKey } from "@/catalog/cell/cube_type.spec.ts";
import { type ActiveTool, Command, encode_input_tracking, type Tracking } from "@/kernel/abi/cmd.spec.ts";
import { CORE_BUILD } from "@/kernel/ipc.ts";
import { boot_kernel } from "@/kernel/kernel.ts";
import { kernel_hold } from "@/kernel/control/input.ts";
import { request_frame, ring_now, strobe } from "@/kernel/sched.ts";
import type { UnsupportedReason } from "@/app/store/unsupported-store.ts";

/** Starts the kernel. A device that runs the app can draw it. */
export async function host_boot(): Promise<UnsupportedReason | null> {
  await boot_kernel();
  return null;
}

/** Sends one of the UI's commands, rung at once (sched.ts `ring_now`). */
export function host_command(cmd: Command, pkt?: U8Array): void {
  ring_now(cmd, pkt);
}

/** The tracking button. */
export function host_tracking(tracking: Tracking): Promise<void> {
  CORE_BUILD.send(0, Command.INPUT_TRACKING, encode_input_tracking({ tracking }));
  strobe.input();
  request_frame();
  return Promise.resolve();
}

/** The sensor button. */
export function host_enable_sensors(): void {
  host_command(Command.SENSOR_ENABLE);
}

/** What the pointer holds, which crosses to the kernel with its events. */
export function host_hold(tool: ActiveTool, channel: GroundChannelKey | null, value: u32, tier: u32): void {
  kernel_hold(tool, channel, value, tier);
}
