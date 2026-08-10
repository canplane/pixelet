// @pxl/shared/spec/space/pvf3/fvec.ts

/**
 * PVF3 (Packed 3-Vector w/ Fixed-Point Arithmetic)
 * Fractional Residue Precision System
 *
 * This module provides atomic, stateless operations for the packed fractional
 * residue layer within the PVF3 coordinate system.
 *
 * Offset Architecture (Internal Resolution), each lane's own MSB is its
 * guard bit (see Guard-Bit Headroom below):
 * [ 31 .. 30 | 29 | 28 ...... 20 | 19 | 18 ...... 10 | 9 | 8 ....... 0 ]
 * [ Reserved | Gx |  Res X (9b)  | Gy |  Res Y (9b)  | Gz|  Res Z (9b) ]
 * [  Unused  |    |  Residue X   |    |  Residue Y   |   |  Residue Z  ]
 *
 * Guard-Bit Headroom (mirrors ivec.ts's own convention, see its module doc):
 * - Each 10-bit lane's legitimate/normalized residue is conventionally bounded to
 *   9 bits (|value| < 2^9 = 512) — one bit narrower than the field. This is a
 *   SEPARATE reservation from the 2-bit "Reserved" region above, which sits once
 *   at the whole word's own top rather than once per lane.
 * - Reserved purely as headroom for unspecified future use, same reasoning as
 *   ivec.ts — NOT because it's been shown to enable a packed-word optimization
 *   here. See this package's README for why (fvec's lanes are UNSIGNED, so the
 *   specific carry-bleed failure mode differs in the details from ivec's signed
 *   case, but the underlying conclusion — packed add/sub isn't worth it for a
 *   fixed 3-lane BigInt word — holds the same for both).
 * - At u ≈ 4.778m, 9 usable bits alone (ignoring ivec's own contribution) already
 *   gives ~4.778m / 2^9 ≈ 0.933cm resolution — comfortably sub-centimeter.
 *
 * Operational Integrity & Masking Policy:
 * - Deterministic Isolation: Each lane is strictly bound to its bit-field
 *   via masking during assembly (gather). Cross-lane leakage is impossible.
 * - Bit-level Parity: Uses BigInt for bit-exact operations to eliminate
 *   floating-point jitter and ensure cross-platform reproducibility.
 * - Residue Wrapping: Per-lane overflow is truncated modulo 2^FRAC_W.
 * - Carry Policy: Integer carry/borrow across lattice boundaries is out of scope;
 *   callers must handle cross-boundary semantics before injecting values here.
 *
 * Mathematical Domain:
 * - Lanes are unsigned fractional residues in [0, 2^FRAC_W), i.e. [0, ONE).
 * - They do not encode sign; signed direction belongs to ivec.
 * - Arithmetic and comparisons operate in the residue domain and therefore
 *   follow wrapped modular behavior after gather().
 *
 * Real Mapping:
 * - Each residue maps to a normalized real in [0, 1).
 * - This gives O(1) sub-lattice positioning independent of physical scale.
 *
 * Format Scalability:
 * - Width-parametric packing: WORD_W and FRAC_W define the format.
 * - Higher fractional precision is achieved by increasing FRAC_W without
 *   changing the arithmetic surface.
 *
 * @module fvec
 */

import { type f64, FIELD_MASK64, type i32, type i64, type u32, type u64 } from "@xpute/core/kit/word.ts";
import type { BigTypedArray } from "@xpute/core/kit/array.ts";

import { FRAC_W, FVEC_WORD_MASK as WORD_MASK, ONE, ONE_MASK } from "./core.ts";
import { EQ, GT, LT, MAX, MIN } from "./core.ts";

/**
 * FVec: PVF3-Packed Fractional Residue Word (Nominal Type)
 * Represents packed unsigned fractional residues within a single lattice unit.
 */
export type FVec = u64;

// ---- Bit Layout Configuration (Symmetric Lane-based) ----

/**
 * SHAMT: Bit-shift alignment for Z-Y-X sequential packing.
 * Calculated internally based on the core specification.
 */
const Z_SHAMT: u32 = 0;
const Y_SHAMT: u32 = Z_SHAMT + FRAC_W;
const X_SHAMT: u32 = Y_SHAMT + FRAC_W;

const _Z_SHAMT: u64 = BigInt(Z_SHAMT);
const _Y_SHAMT: u64 = BigInt(Y_SHAMT);
const _X_SHAMT: u64 = BigInt(X_SHAMT);

// ---- Component Axis Isolation Masks ----

const X_MASK: u64 = FIELD_MASK64(FRAC_W, X_SHAMT);
const Y_MASK: u64 = FIELD_MASK64(FRAC_W, Y_SHAMT);
const Z_MASK: u64 = FIELD_MASK64(FRAC_W, Z_SHAMT);

/**
 * Axis Getters: Extract residue lanes.
 * Returned as i64 for scalar-surface uniformity;
 * semantic domain remains [0, ONE).
 */
export const X = (v: FVec): i64 => (v & X_MASK) >> _X_SHAMT;
export const Y = (v: FVec): i64 => (v & Y_MASK) >> _Y_SHAMT;
export const Z = (v: FVec): i64 => (v & Z_MASK) >> _Z_SHAMT;

/**
 * Fractional residue scalar <-> normalized real.
 * - to_real:   residue scalar -> [0, 1)
 * - from_real: real -> [0, 1) modulo 1 -> residue scalar
 */
export const to_real = (comp: i64): f64 => Number(comp & ONE_MASK) / Number(ONE);
export const from_real = (real: f64): i64 => {
  const alpha = ((real % 1) + 1) % 1;
  return BigInt(Math.floor(Number(ONE) * alpha));
};

// ---- Logging ----

/**
 * Human-readable representation of a packed FVec.
 * Purely for logging and debugging; not for runtime logic.
 */
export const repr = (v: FVec): string => `${X(v)}:${Y(v)}:${Z(v)}`;

// ---- Construction ----

/** Creates a new vector with all components initialized to a uniform value. */
export const fill = (e: i64): FVec => gather(e, e, e);

/**
 * Assembles discrete components into a packed bit-field word.
 * Strict masking (& MASK) ensures no bit leakage between component fields.
 */
export const gather = (x: i64, y: i64, z: i64): FVec => {
  const xn = (x << _X_SHAMT) & X_MASK;
  const yn = (y << _Y_SHAMT) & Y_MASK;
  const zn = (z << _Z_SHAMT) & Z_MASK;
  return xn | yn | zn;
};

/** Deconstructs a packed vector into its constituent components. */
export const scatter = (out: BigTypedArray | i64[], v: FVec): void => {
  out[0] = X(v);
  out[1] = Y(v);
  out[2] = Z(v);
};

// ---- Bitwise ----

export const not = (v: FVec): FVec => (~v) & WORD_MASK;
export const and = (v: FVec, w: FVec): FVec => v & w;
export const or = (v: FVec, w: FVec): FVec => v | w;
export const xor = (v: FVec, w: FVec): FVec => v ^ w;

/**
 * Signed shift: +k => shl, -k => shr
 * - fvec lanes are UNSIGNED residues, so right shift is effectively logical
 *   (BigInt has no >>>; but X/Y/Z are non-negative so >> behaves as logical here).
 */
const shl = (v: FVec, shamt: i32): FVec => {
  const _shamt: i64 = BigInt(shamt);
  return gather(X(v) << _shamt, Y(v) << _shamt, Z(v) << _shamt);
};
const shr = (v: FVec, shamt: i32): FVec => {
  const _shamt: i64 = BigInt(shamt);
  return gather(X(v) >> _shamt, Y(v) >> _shamt, Z(v) >> _shamt);
};
export const sh = (v: FVec, shamt: i32): FVec => shamt >= 0 ? shl(v, shamt) : shr(v, -shamt);

// ---- Arithmetic ----
// NOTE(fvec):
// - lanes are UNSIGNED residues in [0, 2^FRAC_W), i.e. [0, ONE).
// - signed-like neg/abs are intentionally omitted from this surface.
// - if signed semantics are needed, use ivec or reconstruct a higher-level scalar first.
// export const neg = (v: FVec): FVec => gather(-X(v), -Y(v), -Z(v)); // omitted: residue domain has no signed negation surface
// export const abs = (v: FVec): FVec => gather(ABS(X(v)), ABS(Y(v)), ABS(Z(v))); // omitted: residue domain has no signed absolute-value surface

export const add = (v: FVec, w: FVec): FVec => gather(X(v) + X(w), Y(v) + Y(w), Z(v) + Z(w));
export const sub = (v: FVec, w: FVec): FVec => gather(X(v) - X(w), Y(v) - Y(w), Z(v) - Z(w));
export const mul = (v: FVec, w: FVec): FVec => gather(X(v) * X(w), Y(v) * Y(w), Z(v) * Z(w));
export const div = (v: FVec, w: FVec): FVec => gather(X(v) / X(w), Y(v) / Y(w), Z(v) / Z(w));
export const mod = (v: FVec, w: FVec): FVec => gather(X(v) % X(w), Y(v) % Y(w), Z(v) % Z(w));

// ---- Comparison ----

export const eq = (v: FVec, w: FVec): FVec => gather(EQ(X(v), X(w)), EQ(Y(v), Y(w)), EQ(Z(v), Z(w)));
export const equals = (v: FVec, w: FVec): boolean => v === w;
export const lt = (v: FVec, w: FVec): FVec => gather(LT(X(v), X(w)), LT(Y(v), Y(w)), LT(Z(v), Z(w)));
export const gt = (v: FVec, w: FVec): FVec => gather(GT(X(v), X(w)), GT(Y(v), Y(w)), GT(Z(v), Z(w)));
export const max_comp = (v: FVec): i64 => MAX(MAX(X(v), Y(v)), Z(v));
export const min_comp = (v: FVec): i64 => MIN(MIN(X(v), Y(v)), Z(v));
export const max = (v: FVec, w: FVec): FVec => gather(MAX(X(v), X(w)), MAX(Y(v), Y(w)), MAX(Z(v), Z(w)));
export const min = (v: FVec, w: FVec): FVec => gather(MIN(X(v), X(w)), MIN(Y(v), Y(w)), MIN(Z(v), Z(w)));
export const clamp = (v: FVec, min: i64, max: i64): FVec => gather(MIN(MAX(X(v), min), max), MIN(MAX(Y(v), min), max), MIN(MAX(Z(v), min), max));

// ---- Reduction ----
// NOTE(fvec):
// - These reduction helpers are residue-domain proxies, not geometric metrics.
// - norm1 is identical to the component sum in the unsigned residue domain.
// - dist1/dist_sq operate over wrapped subtraction, so they should not be
//   interpreted as canonical spatial distances.
// - these helpers may also be used as local L0-in-L1 slot metrics/orderings
//   in subcell traversal or SVO-style local indexing, not only as numeric proxies.

export const sum = (v: FVec): i64 => X(v) + Y(v) + Z(v);
export const dot = (v: FVec, w: FVec): i64 => sum(mul(v, w));
export const norm1 = (v: FVec): i64 => X(v) + Y(v) + Z(v); // residue-domain proxy: identical to component sum
export const norm_sq = (v: FVec): i64 => dot(v, v);
export const dist1 = (v: FVec, w: FVec): i64 => norm1(sub(v, w)); // residue-domain proxy over wrapped subtraction
export const dist_sq = (v: FVec, w: FVec): i64 => norm_sq(sub(v, w));
