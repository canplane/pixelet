// ts/example/asker.spec.ts
//
// GENERATED from rust/example/asker.json

/**
 * The example's one protocol: the runtime sends SUM, and the core calls ASK as it adds.
 */

import type {
  BranchCursor,
  PacketRoom,
  PacketWriter,
} from "@xpute/kit/wire/xtp/mod.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import * as Command from "./asker/command.spec.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import type { Sender, OnImport } from "@xpute/runtime";

/** One of Command's, as its enum was a type. */
type Command = (typeof Command)[keyof typeof Command];
export { Command };

/** Command.SUM's packet, written in place into `to`. */
export function encode_sum(to: PacketRoom, a: u32, b: u32): PacketWriter {
  return to.writer(2).u32(a).u32(b);
}

/** Command.ASK's packet. */
export interface Ask {}

export function read_ask(_b: BranchCursor): Ask {
  return {};
}

/** What the runtime sends on the asker protocol, bound to `runtime`: each
 * command sent, requested and answered later, or called and answered before
 * it returns. */
export function asker_exports(runtime: Sender) {
  return {
    sum: (a: u32, b: u32): i32 =>
      runtime.call(Command.SUM, (r) => encode_sum(r, a, b)),
  };
}

/** The asker protocol's stubs, bound. */
export type AskerExports = ReturnType<typeof asker_exports>;

/** What the program does with each command the core posts on the asker protocol; a
 * call's answer is its return. */
export interface AskerImports {
  /** Asks the runtime for a number, which the core adds to the sum. */
  ask(): i32;
}

/** `h` as the handler a runtime routes the asker protocol's commands to, each one read and
 * handed to its member; ENOSYS for one the protocol does not carry. */
export function asker_imports(h: AskerImports): OnImport {
  return (_tag, cmd, _f) => {
    switch (cmd) {
      case Command.ASK:
        return h.ask();
      default:
        return Errno.ENOSYS;
    }
  };
}
