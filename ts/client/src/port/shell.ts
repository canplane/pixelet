// @pxl/client/port/shell.ts

/** The kernel as the shell reaches it, and what the drivers below it show the
 * UI. The shell names no host and no driver (AGENTS.md), so a function here
 * that only forwards to one is that boundary, not a rename. */

import type { u32 } from "@xpute/kit/abi/word.ts";
import type {
  NodeValue,
  NodeView,
  PacketRoom,
  PacketWriter,
} from "@xpute/kit/wire/xtp/mod.ts";
import {
  Command,
  encode_input_tracking,
  encode_setting_set,
  Tracking,
} from "@/abi/cmd.spec.ts";
import { HOST } from "@/host/mount.ts";
import * as port from "@/abi/port.spec.ts";
import { bracket } from "@/bracket.tsx";
import { send_input } from "./input.ts";

export { kernel_snapshot } from "@/port/snapshot.ts";
export { kernel_hold, kernel_keys } from "./input.ts";

// Where the client's boot stands, and the stop code of a kernel that stopped.
export { boot_state, watch_boot } from "@/abi/boot.ts";
export { stop_code } from "@/driver/stop.ts";

// What the drivers show the UI: the GPU's state, the network's, the world's
// source, and where the origin serves what the shell fetches itself.
export { gpu, watch_gpu } from "@/driver/gpu.ts";
export { online, watch_online } from "@/driver/namespace/online.ts";
export {
  source_state,
  source_versions,
  watch_source,
} from "@/driver/namespace/source.ts";
export { asset_url } from "@/driver/namespace/route.ts";
export { SEARCH_URL } from "@/driver/namespace/route.spec.ts";
export { INGESTED, PACK } from "@/driver/namespace/ingested.spec.ts";

/** iOS Safari's compass prompt runs here, inside the press. */
export async function kernel_tracking(tracking: Tracking): Promise<void> {
  if (tracking === Tracking.Follow && !(await bracket.sensor.orient.permit())) {
    console.error("[orientation] could not start the sensor.");
    return;
  }
  send_input(
    Command.INPUT_TRACKING,
    encode_input_tracking(HOST.doorbell, tracking),
  );
}

/** A command of the kernel's, rung at once with no quota. */
export function kernel_command(cmd: Command, pkt?: PacketWriter): void {
  HOST.now(port.SHELL, cmd, pkt);
}

export function kernel_setting(key: u32, value: u32): void {
  HOST.now(
    port.SHELL,
    Command.SETTING_SET,
    encode_setting_set(HOST.doorbell, key, value),
  );
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

/** A development console's `cmd`: any command by its number, its arguments
 * its packet's fields, sent as the shell and rung. */
export const kernel_console = {
  cmd: (cmd: u32, ...args: NodeValue<NodeView>[]): void =>
    HOST.doorbell.cmd(port.SHELL, cmd, ...args),
};
