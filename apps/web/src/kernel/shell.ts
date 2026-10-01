// @pxl/web/kernel/shell.ts

/**
 * What the UI reads of the kernel and what it asks of it
 * (crates/pxl/web/kernel/src/shell/shell.rs): the snapshot it renders
 * (snapshot.ts), and the commands its controls send. A control's command is
 * rung at once with no quota, so what it asks of the host is heard inside the
 * call.
 */

import { Command, encode_input_tracking, Tracking } from "@/kernel/abi/cmd.spec.ts";
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

/** The location card: the location alone. The compass's prompt is raised
 * where the compass is used, entering follow mode (kernel_tracking). */
export function kernel_enable_location(): void {
  ring_now(Command.SENSOR_ENABLE_LOCATION);
}
