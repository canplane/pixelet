// @pxl/client/abi/shell.ts

/** The kernel as the shell reaches it. The shell names no host (AGENTS.md),
 * so a function here that only forwards to it is that boundary, not a rename. */

import type { u32 } from "@xpute/kit/abi/word.ts";
import type { Doorbell } from "@xpute/host/ipc/doorbell.ts";
import type { PacketRoom, PacketWriter } from "@xpute/kit/wire/xtp/mod.ts";
import {
  Command,
  encode_input_tracking,
  encode_setting_set,
  Tracking,
} from "@/abi/cmd.spec.ts";
import { HOST } from "@/host/mount.ts";
import { bracket } from "@/bracket.tsx";

export { kernel_snapshot } from "@/abi/snapshot.ts";
export { kernel_hold, kernel_keys } from "./input.ts";

/** iOS Safari's compass prompt runs here, inside the press. */
export async function kernel_tracking(tracking: Tracking): Promise<void> {
  if (tracking === Tracking.Follow && !(await bracket.sensor.orient.permit())) {
    console.error("[orientation] could not start the sensor.");
    return;
  }
  HOST.input(
    Command.INPUT_TRACKING,
    encode_input_tracking(HOST.doorbell, tracking),
  );
}

/** A command of the kernel's, rung at once with no quota. */
export function kernel_command(cmd: Command, pkt?: PacketWriter): void {
  HOST.now(cmd, pkt);
}

export function kernel_setting(key: u32, value: u32): void {
  HOST.now(Command.SETTING_SET, encode_setting_set(HOST.doorbell, key, value));
}

/** Where the shell writes a command's packet (`encode_*`), in place: the
 * doorbell, looked up at each write, since it is made at boot. */
export const kernel_room: PacketRoom = {
  writer: (count) => HOST.doorbell.writer(count),
};

/** Whether a command carrying `pkt` can be sent; the shell refuses input that cannot. */
export function kernel_fits(pkt: PacketWriter): boolean {
  return HOST.doorbell.fits(pkt);
}

/** The guest's doorbell itself, for a development console's `cmd`. */
export function kernel_doorbell(): Doorbell {
  return HOST.doorbell;
}
