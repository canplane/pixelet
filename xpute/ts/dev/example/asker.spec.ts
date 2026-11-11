// ts/dev/example/asker.spec.ts
//
// GENERATED from rust/dev/example/asker.json

/**
 * The example's one protocol: the runtime sends SUM, and the core calls ASK as it adds.
 */

import * as Command from "./asker/command.spec.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import type { Link, Protocol, OnImport } from "@xpute/runtime";
import { refusal_of } from "@xpute/runtime";

/** One of Command's, as its enum was a type. */
type Command = (typeof Command)[keyof typeof Command];
export { Command };

/** Command.ASK's packet. */
export interface Ask {}

/** The asker protocol's exports as the UI calls them over `link`: a send returns
 * nothing, and a request a Promise of its reply's value, which rejects with the
 * errno it was refused with. `fits` says whether a send's packet fits a frame. */
export function asker_exports(link: Link) {
  return {
    /** Carries two numbers. The core answers with their sum plus the number ASK answers. */
    sum: (a: u32, b: u32) => link.request(Command.SUM, a, b),
  };
}

/** The asker protocol's exports, as `asker_exports` binds them. */
export type AskerExports = ReturnType<typeof asker_exports>;

/** What the program does with each command the core posts on the asker protocol: a
 * call's member answers its value, and throws kit's error to refuse it. */
export interface AskerImports {
  /** Asks the runtime for a number, which the core adds to the sum. */
  [Command.ASK](): u32;
}

/** `h` as the protocol the runtime routes the asker protocol's majors to: each
 * command read and handed to its member, whose refusal is answered with its
 * errno; ENOSYS for one the protocol does not carry. */
export function asker_imports(h: AskerImports): Protocol {
  const on: OnImport = (_tag, cmd, _f) => {
    try {
      switch (cmd) {
        case Command.ASK:
          return h[Command.ASK]();
        default:
          return Errno.ENOSYS;
      }
    } catch (err) {
      return refusal_of(err);
    }
  };
  return { majors: [0x10], on };
}
