// @xpute/core/wire/lower.ts

/**
 * JS-side lowering helpers shared by packet/tree frontends.
 *
 * Scope
 * -----
 * - Defines the accepted high-level JS value domain for generic lowering.
 * - Provides plain-record detection for structural lowering.
 * - Provides deterministic key ordering for object-to-branch lowering.
 * - Provides record/tuple conversion helpers for schema-owned ordering.
 *
 * Notes
 * -----
 * - This module is intentionally format-adjacent, but not wire-format-specific.
 * - Arrays preserve source order.
 * - Plain-object keys are ordered:
 *   1) integer-like keys ascending numerically
 *   2) string keys ascending lexicographically
 * - Class instances and exotic host objects are excluded.
 * - Value-domain restrictions such as `undefined` rejection are enforced by
 *   higher layers, not by this module.
 */

import type { primitive } from "@xpute/core/abi/word.ts";
import type { BigTypedArray, TypedArray } from "@xpute/core/abi/array.ts";

export type LowerValue =
  | primitive
  | TypedArray
  | BigTypedArray
  | LowerValue[]
  | LowerRecord;

export interface LowerRecord {
  [k: string]: LowerValue;
}

export function is_plain_record(x: unknown): x is LowerRecord {
  if (x == null || typeof x !== "object") return false;

  const proto = Object.getPrototypeOf(x);
  return proto === Object.prototype || proto === null;
}

export function is_int_like_key(s: string): boolean {
  const n = Number(s);
  return Number.isInteger(n) && String(n) === s;
}

export function cmp_lower_key(a: string, b: string): number {
  const a_int = is_int_like_key(a);
  const b_int = is_int_like_key(b);

  if (a_int && b_int) return Number(a) - Number(b);
  if (a_int) return -1;
  if (b_int) return 1;
  return a < b ? -1 : a > b ? 1 : 0;
}

// ---- Tuple ----

// record -> tuple
export function to_tuple<T extends Record<PropertyKey, unknown>>(
  obj: T,
  keys: readonly (keyof T)[],
): T[keyof T][] {
  const len = keys.length;
  const out: T[keyof T][] = new Array(len);

  for (let i = 0; i < len; i++) {
    const key = keys[i];
    if (!(key in obj)) throw new Error(`missing key in obj: ${String(key)}`);
    out[i] = obj[key];
  }
  return out;
}

// tuple -> record
export function from_tuple<T extends Record<PropertyKey, unknown>>(
  vals: readonly unknown[],
  keys: readonly (keyof T)[],
): T {
  if (vals.length !== keys.length) {
    throw new Error(`tuple length mismatch: expected ${keys.length}, got ${vals.length}`);
  }

  const out = {} as T;
  for (let i = 0; i < keys.length; i++) {
    out[keys[i]] = vals[i] as T[keyof T];
  }
  return out;
}
