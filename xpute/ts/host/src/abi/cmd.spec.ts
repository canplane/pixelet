// @xpute/host/abi/cmd.spec.ts
//
// GENERATED from spec/abi/cmd.json

/**
 * xpute's own commands, by number (kit abi/cmd: `major << 8 | minor`), and the packet
 * each carries — an XTP branch whose children are its fields, in order. A command with
 * no fields carries no packet. The host sends them, port 0 (ipc/frame.json HOST); a
 * program numbers its own as it likes, and the sender tells the two apart.
 *
 * The direction is the ring: `pkt` is what the host submits, `event` what the guest
 * raises on the completion ring.
 */

import type { PacketRoom, PacketWriter } from "@xpute/kit/wire/xtp/mod.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import * as Major from "./cmd/major.spec.ts";
import * as SysMinor from "./cmd/sys_minor.spec.ts";
import * as Command from "./cmd/command.spec.ts";

/** One of Major's, as its enum was a type. */
type Major = (typeof Major)[keyof typeof Major];
export { Major };

/** One of SysMinor's, as its enum was a type. */
type SysMinor = (typeof SysMinor)[keyof typeof SysMinor];
export { SysMinor };

/** One of Command's, as its enum was a type. */
type Command = (typeof Command)[keyof typeof Command];
export { Command };

/** Command.NOP's packet, written in place into `to`. */
export function encode_nop(to: PacketRoom, val: u32): PacketWriter {
  return to.writer(1).u32(val);
}

/** Command.SYS_OPENED's packet, written in place into `to`.
 *
 * @param detail Whatever number the host refused with — for HTTP its status — kept for a reader to print and for nothing to branch on. */
export function encode_sys_opened(
  to: PacketRoom,
  fd: u32,
  res: i32,
  detail: u32,
): PacketWriter {
  return to.writer(3).u32(fd).i32(res).u32(detail);
}
