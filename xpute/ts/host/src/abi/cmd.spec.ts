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

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import { packet } from "@xpute/host/ipc/doorbell.ts";

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

/** Command.NOP's packet. */
export interface Nop {
  value: u32;
}

export function encode_nop(p: Nop): U8Array {
  return packet((b) => b.u32(p.value));
}

/** Command.SYS_OPENED's packet. */
export interface SysOpened {
  fd: u32;
  res: i32;
  /** Whatever number the host refused with — for HTTP its status — kept for a reader to print and for nothing to branch on. */
  detail: u32;
}

export function encode_sys_opened(p: SysOpened): U8Array {
  return packet((b) => b.u32(p.fd).i32(p.res).u32(p.detail));
}
