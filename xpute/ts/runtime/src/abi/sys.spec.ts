// @xpute/runtime/abi/sys.spec.ts
//
// GENERATED from spec/abi/sys.json

/**
 * xpute's own protocol on majors NOP and SYS: the core's life as its runtime tells it, its
 * turns, and its end. NOP is the rings' own test. SYS_ATTACH, SYS_SUSPEND and SYS_RESUME
 * are the core's life as its runtime sees it: a runtime (re)attached, the core about to be
 * frozen, and woken; what each means to keep or reset is the core's. SYS_TURN marks a turn
 * the runtime's scheduling policy grants, with the wall clock. SYS_ABORT is the core's end, the
 * broken invariant it reports as it stops, which the runtime hears as a VMM hears a core's
 * exit. Each command is `major << 8 | minor` (kit abi/cmd) and carries an XTP branch whose
 * children are its fields, in order; one with no fields carries no packet. The GPU and the
 * files are devices of their own (gpu.json, fs.json). Majors 0x00 to 0x0F are xpute's
 * (ipc/frame.json); a program numbers its own protocols' commands past them.
 */

import type {
  BranchCursor,
  PacketRoom,
  PacketWriter,
} from "@xpute/kit/wire/xtp/mod.ts";
import type { f64, i32, u32 } from "@xpute/kit/abi/word.ts";
import * as Major from "./sys/major.spec.ts";
import * as SysMinor from "./sys/sys_minor.spec.ts";
import * as Command from "./sys/command.spec.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import type { Sender, OnImport } from "../mod.ts";

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

/** Command.SYS_TURN's packet, written in place into `to`. */
export function encode_sys_turn(to: PacketRoom, unix_ms: f64): PacketWriter {
  return to.writer(1).f64(unix_ms);
}

/** Command.SYS_ABORT's packet. */
export interface SysAbort {
  errnum: i32;
  line: u32;
  column: u32;
  a_low: u32;
  a_high: u32;
  b_low: u32;
  b_high: u32;
  file: string;
}

export function read_sys_abort(b: BranchCursor): SysAbort {
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

/** What the runtime sends on the sys protocol, bound to `runtime`: each
 * command sent, requested and answered later, or called and answered before
 * it returns. */
export function sys_exports(runtime: Sender) {
  return {
    nop: (val: u32): void =>
      runtime.send(Command.NOP, (r) => encode_nop(r, val)),
    sys_attach: (): void => runtime.send(Command.SYS_ATTACH),
    sys_suspend: (): i32 => runtime.call(Command.SYS_SUSPEND),
    sys_resume: (): i32 => runtime.call(Command.SYS_RESUME),
    sys_turn: (unix_ms: f64): void =>
      runtime.send(Command.SYS_TURN, (r) => encode_sys_turn(r, unix_ms)),
  };
}

/** The sys protocol's stubs, bound. */
export type SysExports = ReturnType<typeof sys_exports>;

/** What the program does with each command the core posts on the sys protocol; a
 * call's answer is its return. */
export interface SysImports {
  /** The broken invariant the core reports as it stops (xpute-kit status/bug.rs). */
  sys_abort(e: SysAbort): void;
}

/** `h` as the handler a runtime routes the sys protocol's commands to, each one read and
 * handed to its member; ENOSYS for one the protocol does not carry. */
export function sys_imports(h: SysImports): OnImport {
  return (_tag, cmd, f) => {
    switch (cmd) {
      case Command.SYS_ABORT:
        return h.sys_abort(read_sys_abort(need(f)));
      default:
        return Errno.ENOSYS;
    }
  };
}

/** A packet the command carries, which a frame without one breaks. */
function need<T>(f: T | null): T {
  if (f === null) throw new MarshalError(Errno.EBADMSG);
  return f;
}
