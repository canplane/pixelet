// @xpute/link/abi/types.ts
//
// GENERATED from link/wit (xpute-link-gen wit.rs)

import type { Lift, Lower } from "../cabi.ts";
import { ERRNO_EBADMSG } from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";

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
  throw new MarshalError(ERRNO_EBADMSG);
}

export interface Lent {
  at: number;
  len: number;
}

export const LENT_SZ = 8;
export const LENT_ALIGN = 4;

export function store_Lent(w: Lower, at: number, v: Lent): void {
  w.u32(at + 0, v.at);
  w.u32(at + 4, v.len);
}

export function load_Lent(r: Lift, at: number): Lent {
  return {
    at: r.u32(at + 0),
    len: r.u32(at + 4),
  };
}
