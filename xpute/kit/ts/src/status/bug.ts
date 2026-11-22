// @xpute/kit/status/bug.ts

/** A broken invariant is a bug, as Rust's `bug!` has it: `bug` throws a
 * `XputeBug`, its errno, the place in the source it was made, at most two
 * numbers and an optional message, as Rust's and C++'s carry. The message is
 * a string literal, never one built at the bug, as theirs is a static string:
 * what varies goes in the two numbers; one past the cap is cut as the bug is
 * made (`cut_msg`), and an empty one is none. A `XputeBug` is no
 * `XputeError`, so a catch that takes an `XputeError` passes it on. */

import { frames, strerror } from "./error.ts";
import { cut_msg } from "./msg.ts";
import type { i32, u32, u64 } from "../abi/word.ts";

export class XputeBug extends Error {
  readonly errnum: i32;
  readonly file: string;
  readonly line: u32;
  readonly a: u64;
  readonly b: u64;
  /** The message its maker gave, or null where it gave none
   * (`XputeError.msg`). `message` is the text shown, this or `strerror` of
   * its errno, as Python's `LogRecord.msg` stands beside `getMessage()`. */
  readonly msg: string | null;

  constructor(
    errnum: i32,
    file: string,
    line: u32,
    a: u64,
    b: u64,
    msg: string | null = null,
  ) {
    const cut = msg === null ? "" : cut_msg(msg);
    super(cut === "" ? strerror(errnum) : cut);
    this.name = "XputeBug";
    this.errnum = errnum;
    this.file = cut_msg(file);
    this.line = line;
    this.a = a;
    this.b = b;
    this.msg = cut === "" ? null : cut;
  }
}

/** Throws the bug `errnum` with the numbers `a` and `b` and what a person
 * reads of it, made where `bug` is called: its caller's frame, the one after
 * `bug`'s own, as Rust's `#[track_caller]` gives it. */
export function bug(
  errnum: i32,
  a: u64 = 0n,
  b: u64 = 0n,
  msg: string | null = null,
): never {
  const [file, line] = frames(new Error().stack ?? "")[1] ?? ["", 0];
  throw new XputeBug(errnum, file, line, a, b, msg);
}
