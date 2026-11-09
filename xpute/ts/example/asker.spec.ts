// ts/example/asker.spec.ts
//
// GENERATED from rust/example/asker.json

/**
 * The example's one protocol: the runtime sends SUM, and the core calls ASK as it adds.
 */

import * as Command from "./asker/command.spec.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import type { Protocol, OnImport } from "@xpute/runtime";

/** One of Command's, as its enum was a type. */
type Command = (typeof Command)[keyof typeof Command];
export { Command };

/** Command.ASK's packet. */
export interface Ask {}

/** What the runtime sends on the asker protocol: each command by its number, and
 * the fields its packet takes in order. It is sent, requested and answered
 * later, or called and answered before it returns. */
export type AskerSend = Record<never, never>;
export type AskerRequest = Record<never, never>;
export type AskerCall = {
  [Command.SUM]: [a: u32, b: u32];
};

/** The asker protocol over a link. */
export type AskerProtocol = Protocol<AskerSend, AskerRequest, AskerCall>;

/** What the program does with each command the core posts on the asker protocol; a
 * call's answer is its return. */
export interface AskerImports {
  /** Asks the runtime for a number, which the core adds to the sum. */
  [Command.ASK](): i32;
}

/** `h` as the handler a runtime routes the asker protocol's commands to, each one read and
 * handed to its member; ENOSYS for one the protocol does not carry. */
export function asker_imports(h: AskerImports): OnImport {
  return (_tag, cmd, _f) => {
    switch (cmd) {
      case Command.ASK:
        return h[Command.ASK]();
      default:
        return Errno.ENOSYS;
    }
  };
}
