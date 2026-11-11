// @xpute/runtime/abi/number.ts

/**
 * A number a packet carries, read as its field's type, as xpute-core
 * abi/number.rs reads one, so that nothing past a reader meets a NaN, an
 * infinity, or an integer forced into range: a number its type does not hold
 * is EINVAL. That the node is a number at all is the cursor's to read
 * (`get_number`, @xpute/kit wire/xtp).
 */

import {
  type f64,
  fits_i32,
  fits_u32,
  fits_u8,
  type i32,
  type u32,
  type u8,
} from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";

const refuse = (): never => {
  throw new MarshalError(Errno.EINVAL);
};

export const FINITE = (n: f64): f64 => (Number.isFinite(n) ? n : refuse());

export const INT_U8 = (n: f64): u8 => (fits_u8(n) ? n : refuse());

export const INT_U32 = (n: f64): u32 => (fits_u32(n) ? n : refuse());

export const INT_I32 = (n: f64): i32 => (fits_i32(n) ? n : refuse());

/** A member of the enum `members`, or `unknown` for a number none is given;
 * with no `unknown`, such a number is EINVAL. */
export function MEMBER<M extends number>(
  n: f64,
  members: Readonly<Record<string, M>>,
  unknown?: M,
): M {
  const m = INT_U32(n);
  if ((Object.values(members) as number[]).includes(m)) return m as M;
  return unknown ?? refuse();
}
