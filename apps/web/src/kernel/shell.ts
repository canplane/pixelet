// @pxl/web/kernel/shell.ts

import type { U8Array } from "@xpute/core/abi/array.ts";
import type { u32 } from "@xpute/core/abi/word.ts";
import { Command, encode_input_tracking, Tracking } from "@/kernel/abi/cmd.spec.ts";
import { CORE, CORE_DOORBELL } from "@/kernel/mem.ts";
import { ring_now, send_input } from "@/kernel/sched.ts";
import { orientationPermit } from "@platform/sensor.ts";

export { kernel_snapshot, useKernel } from "@/kernel/snapshot.ts";
export { kernel_hold } from "@/kernel/control/input.ts";
export { ring_now as kernel_command } from "@/kernel/sched.ts";

/** iOS Safari's compass prompt runs here, inside the press. */
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
