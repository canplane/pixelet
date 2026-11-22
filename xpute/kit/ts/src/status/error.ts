// @xpute/kit/status/error.ts

import {
  KIND_UNKNOWN,
  kind_Errno,
  name_Errno,
  of_Errno,
  text_Errno,
  type Kind,
} from "./errno.spec.ts";
import { cut_msg } from "./msg.ts";
import type { i32, u32 } from "../abi/word.ts";

/** An errno as a reader is shown it: its name and what it means
 * (`errno.json5`). The strings are for a person to read; an errno is its
 * number alone. */
export function strerror(errnum: i32): string {
  const e = of_Errno(errnum);
  return e === null ? `errno ${errnum}` : `${name_Errno(e)}: ${text_Errno(e)}`;
}

/** Each frame of `stack` that names its place, as its file and line, the
 * innermost first. A frame's place ends in `:line:col`; its file is what comes
 * before, after V8's `at f (` or `at `, or after the other engines' `f@`, so a
 * file whose path holds `@` (`node_modules/@xpute/…`) is kept whole. */
export function frames(stack: string): [string, u32][] {
  const out: [string, u32][] = [];
  for (const frame of stack.split("\n")) {
    const m = /^(.*):(\d+):\d+\)?$/.exec(frame.trim());
    if (m === null) continue;
    const at = /^at\s+(?:.*?\()?(.*)$/.exec(m[1]!);
    out.push([
      at !== null ? at[1]! : m[1]!.slice(m[1]!.indexOf("@") + 1),
      Number(m[2]),
    ]);
  }
  return out;
}

/** An errno a fault from outside the program raised: its input, its peer,
 * the network or the environment. It is caught, and what it stops is
 * isolated, degraded or dropped; a broken invariant is a `XputeBug` (`bug.ts`)
 * instead. What happened is its `kind`. It holds the place it was made, its
 * `file` and its `line`, as Rust's and C++'s do, and what its maker says of
 * it. Across the link it goes whole, so the error its caller is thrown is the
 * one that was made, with the place and the message of its maker's side. */
export class XputeError extends Error {
  readonly errnum: i32;
  /** The message its maker gave, or null where it gave none, as Rust's
   * `Option` and C++'s empty view say; one past the cap is cut as it is made
   * (`cut_msg`), and an empty one is none. `message` is the text shown, this
   * or `strerror` of its errno, as Python's `LogRecord.msg` stands beside
   * `getMessage()`. */
  readonly msg: string | null;
  /** The file of the place it was made: where it was constructed, the frame
   * after the constructor's own, as `bug` takes the frame after its own, or
   * what came across the link with an error another side made. A class over
   * this one, or a function that makes an error for its caller, would stand
   * a frame between and take the place. */
  readonly file: string;
  /** The line of the place it was made. */
  readonly line: u32;

  /** The error `errnum` with the message `msg`, made here, where it is
   * constructed, or at `place`, its file and line, where it came across the
   * link from the side that made it; the file is cut as the message is,
   * whatever that side sent. */
  constructor(
    errnum: i32,
    msg: string | null = null,
    place: readonly [string, u32] | null = null,
  ) {
    const cut = msg === null ? "" : cut_msg(msg);
    super(cut === "" ? strerror(errnum) : cut);
    this.name = "XputeError";
    this.errnum = errnum;
    this.msg = cut === "" ? null : cut;
    const [file, line] = place ?? frames(new Error().stack ?? "")[1] ?? ["", 0];
    this.file = cut_msg(file);
    this.line = line;
  }

  /** What happened: the errno's kind (`errno.json5`), or unknown for a
   * number no errno has, as Abseil's `ErrnoToStatusCode` answers one. */
  kind(): Kind {
    const e = of_Errno(this.errnum);
    return e === null ? KIND_UNKNOWN : kind_Errno(e);
  }
}
