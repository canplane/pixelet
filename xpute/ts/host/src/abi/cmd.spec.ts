// @xpute/host/abi/cmd.spec.ts
//
// GENERATED from spec/abi/cmd.json — do not edit.

/**
 * Every program's commands, by number (kit abi/cmd: `major << 8 | minor`), and the
 * packet each carries — an XTP branch whose children are its fields, in order. A
 * command with no fields carries no packet. The majors below PROGRAM_MAJOR are xpute's;
 * a program numbers its own from there, in a table of its own that names this one as
 * its base, so the two never give out one number twice.
 *
 * The direction is the ring: `packet` is what the host submits, `signal` what the guest
 * raises on the completion ring.
 */

import type { PacketRoom, PacketWriter } from "@xpute/kit/wire/xtp/mod.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";

/** What a command is about. */
export const enum Major {
  NOP = 0x00,
  SYS = 0x01,
}

/** The system calls' answers (ipc/sys.json). */
export const enum SysMinor {
  OPENED = 0x01,
}

/** Every program's commands. */
export const enum Command {
  /** Replies with its packet's value, or 0 where it has none — the rings' own test. */
  NOP = 0x0000,
  /** An OPENAT answered: the file's size, or -errno — ENOENT where there is no such file.
   * Never sent for a descriptor already closed. */
  SYS_OPENED = 0x0101,
}

/** Command.NOP's packet, written in place into `to`. */
export function encode_nop(to: PacketRoom, value: u32): PacketWriter {
  return to.writer(1).u32(value);
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
