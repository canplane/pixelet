// @pxl/web/kernel/shell.ts

import type { U8Array } from "@xpute/core/abi/array.ts";
import type { u32 } from "@xpute/core/abi/word.ts";
import { Command, encode_input_tracking, Tracking } from "@/kernel/abi/cmd.spec.ts";
import { CORE, CORE_DOORBELL } from "@/kernel/mem.ts";
import { ring_now, send_input } from "@/kernel/sched.ts";
import { orientationPermit } from "@platform/sensor.ts";

export { kernel_snapshot, useKernel } from "@/kernel/snapshot.ts";
export { kernel_hold } from "@/kernel/control/input.ts";
/** Sends one of the UI's commands, rung at once. */
export { ring_now as kernel_command } from "@/kernel/sched.ts";

/** The tracking button. Following needs the compass, so where a platform
 * gates it behind a prompt (iOS Safari), the prompt runs here, inside the
 * press, and the kernel hears the answer before the press — the way a
 * page's handler awaits `requestPermission` and the browser only reports the
 * outcome. */
export async function kernel_tracking(tracking: Tracking): Promise<void> {
  if (tracking === Tracking.Follow && !(await orientationPermit())) {
    console.error("[orientation] could not start the sensor.");
    return;
  }
  send_input(Command.INPUT_TRACKING, encode_input_tracking({ tracking }));
}

export function kernel_enable_location(): void {
  ring_now(Command.SENSOR_ENABLE_LOCATION);
}

/** The kernel's memory, where a call leaves its reply; a view of it holds until the next turn. */
export function kernel_memory(): ArrayBuffer {
  return CORE.buffer;
}

/** A command answered inside the call: the address of its reply in `kernel_memory()`. */
export function kernel_call(cmd: Command, pkt: U8Array | null = null): u32 {
  return CORE_DOORBELL.call(cmd, pkt, (at) => at >>> 0);
}
