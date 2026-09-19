// @xpute/core/abi/cast.ts

/**
 * Kernel ABI – Bit Casting
 *
 * Re-interprets an existing bit-pattern with a given width & signedness.
 *
 * - NOT serialization
 * - NOT IO
 * - NO validation
 *
 * nbits : Number of bits (field width)
 *   u(...)  : unsigned reinterpretation (number container; 32-bit bitops)
 *   d(...)  : signed reinterpretation   (number container; 32-bit bitops)
 *   lu(...) : unsigned reinterpretation (bigint container; width-parametric)
 *   ld(...) : signed reinterpretation   (bigint container; width-parametric)
 *
 * NOTE
 * ----
 * - number-path assumes 1 <= w <= 32 (JS bitops are 32-bit)
 * - bigint-path assumes 1 <= w
 */

export type nbits = number;

// ============ Number (<= 32-bit containers) ============

/** unsigned reinterpretation (low w bits) */
export const u = (x: number, w: nbits): number => {
  if (w === 32) return x >>> 0;
  return x & ((1 << w) - 1);
};

/** signed reinterpretation (two's complement on w bits) */
export const d = (x: number, w: nbits): number => {
  if (w === 32) return x | 0;

  const m = (1 << w) - 1; // mask for w bits
  const s = 1 << (w - 1); // sign bit in w bits

  x &= m;
  return x & s ? x | ~m : x; // sign-extend within 32-bit lane
};

// ============ Bigint (width-parametric) ============

/** unsigned reinterpretation (low w bits) */
export const lu = (x: bigint, w: nbits): bigint => {
  const W = BigInt(w);
  return x & ((1n << W) - 1n);
};

/** signed reinterpretation (two's complement on w bits) */
export const ld = (x: bigint, w: nbits): bigint => {
  const W = BigInt(w);
  const m = (1n << W) - 1n; // mask for w bits
  const s = 1n << (W - 1n); // sign bit in w bits

  x &= m;
  return x & s ? x - (1n << W) : x; // signed value in bigint domain
};

// ============ Usage Examples (copy/paste mental model) ============

/* * Example 1: 24-bit signed value stored in 32-bit container
  import * as scalar from "../wire/scalar.ts";
  import * as cast from "./cast.ts";

  const raw = scalar.getu32(view, off); // raw bits
  const v   = cast.d(raw, 24);      // interpret low-24 as signed
*/

/* * Example 2: 24-bit signed field inside a bigint container
  const raw = scalar.getu64(view, off);
  const v   = cast.ld(raw, 24);
*/

/* * Example 3: prepare sub-field before store (trim width)
  const x24 = cast.lu(BigInt(x), 24); // keep low-24
  scalar.putu32(view, off, Number(x24));
*/
