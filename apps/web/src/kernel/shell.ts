// @pxl/web/kernel/shell.ts

import { Command, encode_input_tracking, Tracking } from "@/kernel/abi/cmd.spec.ts";
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
