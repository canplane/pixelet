// @xpute/runtime/io/path.ts

/**
 * A file's path (spec/abi/fs.json): an absolute tuple of tokens, each a u32
 * or a string, as many as the program uses. xpute gives no token a meaning;
 * the program's binding does (io/fs.ts).
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";

export type Token = u32 | string;

export type Path = readonly Token[];

/** Reads a path from its branch: each child a string, or a number that is a
 * u32. */
export function read_path(b: BranchCursor): Path {
  return Array.from({ length: b.len }, (_, k) => {
    const v = b.at(k).get();
    if (typeof v === "string") return v;
    if (
      typeof v === "number" &&
      Number.isInteger(v) &&
      v >= 0 &&
      v <= 0xffffffff
    ) {
      return v;
    }
    throw new MarshalError(Errno.EBADMSG);
  });
}
