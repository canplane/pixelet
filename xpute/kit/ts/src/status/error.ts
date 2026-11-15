// @xpute/kit/status/error.ts

import {
  ERRNO,
  KIND_UNKNOWN,
  kind_Errno,
  of_Errno,
  type Kind,
} from "./errno.spec.ts";
import type { i32 } from "../abi/word.ts";

/** An errno as a reader is shown it: its name and what it means
 * (`errno.json5`). The strings are for a person to read; an error carries the
 * number alone. */
export function strerror(errnum: i32): string {
  const e = Object.entries(ERRNO).find(([, v]) => v.number === errnum);
  return e === undefined ? `errno ${errnum}` : `${e[0]}: ${e[1].text}`;
}

/** An errno a fault from outside the program raised: its input, its peer,
 * the network or the environment. It is caught, and what it stops is
 * isolated, degraded or dropped; a broken invariant is a `XputeBug` (`bug.ts`)
 * instead. What happened is its `kind`. Its message is what its maker says
 * of it, or `strerror` of its errno; across the link it is its errno alone. */
export class XputeError extends Error {
  readonly errnum: i32;

  constructor(errnum: i32, message?: string) {
    super(message ?? strerror(errnum));
    this.name = "XputeError";
    this.errnum = errnum;
  }

  /** What happened: the errno's kind (`errno.json5`), or unknown for a
   * number no errno has, as Abseil's `ErrnoToStatusCode` answers one. */
  kind(): Kind {
    const e = of_Errno(this.errnum);
    return e === null ? KIND_UNKNOWN : kind_Errno(e);
  }
}
