// ts/example/asker.spec.ts
//
// GENERATED from rust/example/asker.json

/**
 * The example's one protocol: the host sends SUM, and the guest calls ASK as it adds.
 */

import type {
  BranchCursor,
  PacketRoom,
  PacketWriter,
} from "@xpute/kit/wire/xtp/mod.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import * as Command from "./asker/command.spec.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import type { Host, OnEvent } from "@xpute/host";

/** One of Command's, as its enum was a type. */
type Command = (typeof Command)[keyof typeof Command];
export { Command };

/** Command.SUM's packet, written in place into `to`. */
export function encode_sum(to: PacketRoom, a: u32, b: u32): PacketWriter {
  return to.writer(2).u32(a).u32(b);
}

/** Command.ASK's event. */
export interface AskEvent {}

export function read_ask(_b: BranchCursor): AskEvent {
  return {};
}

/** What the host sends on the asker protocol, bound to `host`: each
 * command rung, or called and answered before it returns. */
export function asker_exports(host: Host) {
  return {
    sum: (a: u32, b: u32): i32 =>
      host.call(Command.SUM, (r) => encode_sum(r, a, b)),
  };
}

/** The asker protocol's stubs, bound. */
export type AskerExports = ReturnType<typeof asker_exports>;

/** What the program does with each command the guest posts on the asker protocol; a
 * call's answer is its return. */
export interface AskerImports {
  /** A number the host answers, which the guest adds. */
  ask(): i32;
}

/** `h` as the handler a host routes the asker protocol's commands to, each one read and
 * handed to its member; ENOSYS for one the protocol does not carry. */
export function asker_imports(h: AskerImports): OnEvent {
  return (_tag, cmd, _f) => {
    switch (cmd) {
      case Command.ASK:
        return h.ask();
      default:
        return Errno.ENOSYS;
    }
  };
}
