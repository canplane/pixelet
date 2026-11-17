// @xpute/link/abi/types.ts
//
// GENERATED from link/wit (xpute-link-gen wit.rs)

import type { Lift, Lower } from "../cabi.ts";
import type { u64 } from "@xpute/kit/abi/word.ts";
import { ERRNO_EBADMSG } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";

export type Token = { tag: "int"; val: number } | { tag: "str"; val: string };

export const TOKEN_SZ = 12;
export const TOKEN_ALIGN = 4;

export function store_Token(w: Lower, at: number, v: Token): void {
  switch (v.tag) {
    case "int":
      w.u8(at, 0);
      w.i32(at + 4, v.val);
      return;
    case "str":
      w.u8(at, 1);
      w.str(at + 4, v.val);
      return;
  }
}

export function load_Token(r: Lift, at: number): Token {
  switch (r.case(at, 2)) {
    case 0:
      return { tag: "int", val: r.i32(at + 4) };
    case 1:
      return { tag: "str", val: r.str(at + 4) };
  }
  throw new XputeError(ERRNO_EBADMSG);
}

export interface Lent {
  buf: u64;
  nbyte: number;
}

export const LENT_SZ = 16;
export const LENT_ALIGN = 8;

export function store_Lent(w: Lower, at: number, v: Lent): void {
  w.u64(at, v.buf);
  w.u32(at + 8, v.nbyte);
}

export function load_Lent(r: Lift, at: number): Lent {
  return {
    buf: r.u64(at),
    nbyte: r.u32(at + 8),
  };
}
