// @xpute/runtime/abi/link.ts
//
// GENERATED from wit (xpute-gen spec/wit.rs)

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { type Arg, type NodeCursor } from "@xpute/kit/wire/xtp/mod.ts";

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
