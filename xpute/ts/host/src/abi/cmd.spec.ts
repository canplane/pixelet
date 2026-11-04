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
 * posts on the completion ring for the host's port, its hostcalls (ipc/sys.json).
 */

import type {
  BranchCursor,
  PacketRoom,
  PacketWriter,
} from "@xpute/kit/wire/xtp/mod.ts";
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

/** Command.SYS_OPENAT's event. */
export interface SysOpenatEvent {
  fd: u32;
  root: u32;
  /** ipc/sys.json `flag`. */
  flags: u32;
  path: string;
}

export function read_sys_openat(b: BranchCursor): SysOpenatEvent {
  return {
    fd: b.at(0).get<u32>(),
    root: b.at(1).get<u32>(),
    flags: b.at(2).get<u32>(),
    path: b.at(3).get<string>(),
  };
}

/** Command.SYS_READ's event. */
export interface SysReadEvent {
  fd: u32;
  at: u32;
  len: u32;
}

export function read_sys_read(b: BranchCursor): SysReadEvent {
  return {
    fd: b.at(0).get<u32>(),
    at: b.at(1).get<u32>(),
    len: b.at(2).get<u32>(),
  };
}

/** Command.SYS_WRITE's event. */
export interface SysWriteEvent {
  fd: u32;
  at: u32;
  len: u32;
}

export function read_sys_write(b: BranchCursor): SysWriteEvent {
  return {
    fd: b.at(0).get<u32>(),
    at: b.at(1).get<u32>(),
    len: b.at(2).get<u32>(),
  };
}

/** Command.SYS_CLOSE's event. */
export interface SysCloseEvent {
  fd: u32;
}

export function read_sys_close(b: BranchCursor): SysCloseEvent {
  return { fd: b.at(0).get<u32>() };
}

/** Command.SYS_ABORT's event. */
export interface SysAbortEvent {
  errnum: i32;
  line: u32;
  column: u32;
  a_low: u32;
  a_high: u32;
  b_low: u32;
  b_high: u32;
  file: string;
}

export function read_sys_abort(b: BranchCursor): SysAbortEvent {
  return {
    errnum: b.at(0).get<i32>(),
    line: b.at(1).get<u32>(),
    column: b.at(2).get<u32>(),
    a_low: b.at(3).get<u32>(),
    a_high: b.at(4).get<u32>(),
    b_low: b.at(5).get<u32>(),
    b_high: b.at(6).get<u32>(),
    file: b.at(7).get<string>(),
  };
}
