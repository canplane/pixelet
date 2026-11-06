// @xpute/runtime/abi/cmd.spec.ts
//
// GENERATED from spec/abi/cmd.json

/**
 * xpute's own protocol, the runtime's, on majors NOP, SYS and GPU: the core's life as its runtime tells it.
 * NOP is the rings' own test. SYS_ATTACH, SYS_SUSPEND and SYS_RESUME are the core's
 * life as its runtime sees it: a runtime (re)attached, the core about to be frozen, and woken;
 * what each means to keep or reset is the core's. GPU_READY is the GPU grant's state:
 * the device the machine asked for answered. SYS_ABORT is the core's end, the broken
 * invariant it reports as it stops, which the runtime hears as a VMM hears a core's exit.
 * Each command is `major << 8 | minor` (kit abi/cmd) and carries an XTP branch whose
 * children are its fields, in order; one with no fields carries no packet. Files are a
 * device of their own (fs.json). Majors 0x00 to 0x0F are xpute's (ipc/frame.json); a
 * program numbers its own protocols' commands past them.
 */

import type {
  BranchCursor,
  PacketRoom,
  PacketWriter,
} from "@xpute/kit/wire/xtp/mod.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import * as Major from "./cmd/major.spec.ts";
import * as SysMinor from "./cmd/sys_minor.spec.ts";
import * as GpuMinor from "./cmd/gpu_minor.spec.ts";
import * as Command from "./cmd/command.spec.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import type { Sender, OnImport } from "../mod.ts";

/** One of Major's, as its enum was a type. */
type Major = (typeof Major)[keyof typeof Major];
export { Major };

/** One of SysMinor's, as its enum was a type. */
type SysMinor = (typeof SysMinor)[keyof typeof SysMinor];
export { SysMinor };

/** One of GpuMinor's, as its enum was a type. */
type GpuMinor = (typeof GpuMinor)[keyof typeof GpuMinor];
export { GpuMinor };

/** One of Command's, as its enum was a type. */
type Command = (typeof Command)[keyof typeof Command];
export { Command };

/** Command.NOP's packet, written in place into `to`. */
export function encode_nop(to: PacketRoom, val: u32): PacketWriter {
  return to.writer(1).u32(val);
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

/** What the runtime sends on the runtime protocol, bound to `runtime`: each
 * command sent, requested and answered later, or called and answered before
 * it returns. */
export function runtime_exports(runtime: Sender) {
  return {
    nop: (val: u32): void =>
      runtime.send(Command.NOP, (r) => encode_nop(r, val)),
    sys_attach: (): void => runtime.send(Command.SYS_ATTACH),
    sys_suspend: (): i32 => runtime.call(Command.SYS_SUSPEND),
    sys_resume: (): i32 => runtime.call(Command.SYS_RESUME),
    gpu_ready: (): void => runtime.send(Command.GPU_READY),
  };
}

/** The runtime protocol's stubs, bound. */
export type RuntimeExports = ReturnType<typeof runtime_exports>;

/** What the program does with each command the core posts on the runtime protocol; a
 * call's answer is its return. */
export interface RuntimeImports {
  /** The broken invariant the core reports as it stops (xpute-kit status/bug.rs). */
  sys_abort(e: SysAbort): void;
}

/** `h` as the handler a runtime routes the runtime protocol's commands to, each one read and
 * handed to its member; ENOSYS for one the protocol does not carry. */
export function runtime_imports(h: RuntimeImports): OnImport {
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
