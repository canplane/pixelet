// @pxl/client/kernel/shell.ts

import type { u32 } from "@xpute/kit/abi/word.ts";
import { FaultError } from "@xpute/kit/status/error.ts";
import type { Errno } from "@xpute/kit/status/errno.spec.ts";
import type { PacketRoom, PacketWriter } from "@xpute/kit/wire/xtp/mod.ts";
import { Command, encode_input_tracking, encode_setting_set, Tracking } from "@/kernel/abi/cmd.spec.ts";
import { GUEST, GUEST_DOORBELL } from "@/kernel/mem.ts";
import { ring_now, send_input } from "@/kernel/sched.ts";
import { orientationPermit } from "@platform/sensor.ts";

export { kernel_snapshot, useKernel } from "@/kernel/snapshot.ts";
export { kernel_hold, kernel_keys } from "@/kernel/control/input.ts";
export { ring_now as kernel_command } from "@/kernel/sched.ts";

/** iOS Safari's compass prompt runs here, inside the press. */
export async function kernel_tracking(tracking: Tracking): Promise<void> {
  if (tracking === Tracking.Follow && !(await orientationPermit())) {
    console.error("[orientation] could not start the sensor.");
    return;
  }
  send_input(Command.INPUT_TRACKING, encode_input_tracking(GUEST_DOORBELL, tracking));
}

export function kernel_setting(key: u32, value: u32): void {
  ring_now(Command.SETTING_SET, encode_setting_set(GUEST_DOORBELL, key, value));
}

/** Where the shell writes a command's packet (`encode_*`), in place: the
 * guest's doorbell as it stands at the write, made at boot. */
export const kernel_room: PacketRoom = { writer: (count) => GUEST_DOORBELL.writer(count) };

/** Whether a command carrying `pkt` can be sent; the shell refuses input that cannot. */
export function kernel_fits(pkt: PacketWriter): boolean {
  return GUEST_DOORBELL.fits(pkt);
}

export function kernel_enable_location(): void {
  ring_now(Command.SENSOR_ENABLE_LOCATION);
}

/** The kernel's memory, where a call leaves its reply; a view of it holds until the next turn. */
export function kernel_memory(): ArrayBuffer {
  return GUEST.buffer;
}

/** A command answered inside the call: the address of its reply in `kernel_memory()`. */
export function kernel_call(cmd: Command, pkt: PacketWriter | null = null): u32 {
  return GUEST_DOORBELL.call(cmd, pkt, (at) => {
    // A refusal is -errno, never an address.
    if (at < 0) throw new FaultError(-at as Errno);
    return at >>> 0;
  });
}
