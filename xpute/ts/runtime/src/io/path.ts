// @xpute/runtime/io/path.ts

/**
 * A file's path (spec/abi/fs.json): an absolute tuple of tokens, each an i32
 * or a string, as many as the program uses. xpute gives no token a meaning;
 * the program's binding does (io/fs.ts).
 */

import { fits_i32, type i32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";

/** A token is an i32 or a string, each of which compares exactly; xpute gives
 * none a meaning. */
export type Token = i32 | string;

export type Path = readonly Token[];

/** Reads a path from its branch: each child a string, or a number whose value
 * is a whole i32, as a whole float's is. Any other child, a fraction, a bool, a
 * branch or an integer past an i32, is EBADMSG. */
export function read_path(b: BranchCursor): Path {
  return Array.from({ length: b.len }, (_, k) => {
    const c = b.at(k);
    const v = c.get();
    if (typeof v === "string") return v;
    const n = c.get_number();
    if (!fits_i32(n)) throw new MarshalError(Errno.EBADMSG);
    return n;
  });
}
