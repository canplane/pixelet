// @xpute/kit/status/bug.ts

/** A broken invariant is a bug, as Rust's `bug!` has it: `bug` throws a
 * `XputeBug`, its errno and the place in the source it was made. A `XputeBug` is no
 * `XputeError`, so a catch that takes an `XputeError` passes it on. */

import { strerror } from "./error.ts";
import type { i32, u32 } from "../abi/word.ts";

export class XputeBug extends Error {
  readonly errnum: i32;
  readonly file: string;
  readonly line: u32;

  constructor(errnum: i32, file: string, line: u32) {
    super(strerror(errnum));
    this.name = "XputeBug";
    this.errnum = errnum;
    this.file = file;
    this.line = line;
  }
}

/** Throws the bug `errnum`, made where `bug` is called: its caller's frame,
 * the one after `bug`'s own, as Rust's `#[track_caller]` gives it. */
export function bug(errnum: i32): never {
  const [file, line] = frames(new Error().stack ?? "")[1] ?? ["", 0];
  throw new XputeBug(errnum, file, line);
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
