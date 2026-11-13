// @xpute/link/abi/types.ts
//
// GENERATED from link/wit (xpute-link-gen wit.rs)

import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { Branch, type Arg, type NodeCursor } from "@xpute/kit/wire/xtp/mod.ts";
import { uri } from "../link.ts";

/** The interface's qualified name: its package and its name. */
export const NAME = uri("xpute:link", "types");

export type Token = { tag: "int"; val: number } | { tag: "str"; val: string };

/** Its payload alone, whose node's type says its case. */
export function write_Token(v: Token): Arg {
  switch (v.tag) {
    case "int":
      return v.val;
    case "str":
      return v.val;
  }
}

export function read_Token(c: NodeCursor): Token {
  if (typeof c.get() === "number") return { tag: "int", val: c.get_number() };
  if (typeof c.get() === "string") return { tag: "str", val: c.get_str() };
  throw new XputeError(Errno.EBADMSG);
}

export interface Lent {
  at: number;
  len: number;
}

export function write_Lent(r: Lent): Branch {
  return new Branch([r.at, r.len]);
}

export function read_Lent(c: NodeCursor): Lent {
  const b = c.as_branch();
  return {
    at: b.at(0).get_number(),
    len: b.at(1).get_number(),
  };
}
