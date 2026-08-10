// @pxl/shared/spec/space/pvf3/ivec.ts

/**
 * PVF3 (Packed 3-Vector w/ Fixed-Point Arithmetic)
 * Integer Component Precision System
 *
 * This module provides atomic, stateless operations for the high-capacity
 * packed integer grid indexing. It handles the primary spatial partitioning
 * and discrete addressing logic for multi-layered coordinate systems.
 *
 * Memory Layout (N-bit Packed Index):
 * [ MSB ........................................................... LSB ]
 * [ Lane X (X_W bits) | Lane Y (Y_W bits) | Lane Z (Z_W bits) ]
 *
 * Example Configuration (64-bit Word), each lane's own MSB is its guard bit
 * (see Guard-Bit Headroom below):
 * [ 63 | 62 ............... 40 | 39 | 38 ............... 16 | 15 | 14 ............... 0 ]
 * [ Gx |    Lane X (23b)       | Gy |    Lane Y (23b)       | Gz |    Lane Z (15b)       ]
 *
 * Mathematical Domain & Topology:
 * - X/Y/Z lanes are interpreted as signed (two's complement).
 * - X is cyclic (wrap-around) in the chosen world model: the signed boundary represents a continuous seam
 *   (i.e., MIN and MAX meet at the seam, e.g., antimeridian behavior) rather than a clamp.
 *
 * Guard-Bit Headroom (why lane widths are one bit wider than the legitimate domain):
 * - A NORMALIZED coordinate never uses a lane's full physical width — each lane's legitimate
 *   magnitude is bounded to (lane_width - 1) bits, one bit narrower than the field it's stored
 *   in (X/Y: |value| < 2^22 inside a 24-bit field; Z: |value| < 2^14 inside a 16-bit field).
 * - For X and Y this headroom isn't a policy choice, it falls out of the coordinate domain
 *   itself: EPSG:3857's own square bounds put both axes' legitimate range at essentially the
 *   same magnitude (see adapter/3857.ts's WORLD_WIDTH_3857-derived U_PER_M) — the field being
 *   one bit wider than that was already true before this was ever named as a convention. For Z
 *   there's no equivalent domain-forced ceiling (altitude has no analog to a Mercator square
 *   bound); its own value happens to sit nowhere near even a much tighter ceiling than this.
 * - X additionally benefits from this same margin for an unrelated reason: because its seam is
 *   a wrap (not a clamp), a delta computed against a not-yet-rewrapped position near the seam
 *   can transiently swing by close to a full world-width before whatever re-wraps it runs, and
 *   this headroom is what keeps that transient from corrupting the value before it's corrected.
 * - What this headroom does NOT buy: a single raw `(v + w) & WORD_MASK` across the whole packed
 *   word, skipping gather()'s per-lane unpack/repack. That was tried and measured wrong — see
 *   this package's README for the full investigation.
 *
 * Operational Integrity & Overflow Policy:
 * - Deterministic Arithmetic: Uses BigInt for bit-exact operations; per-lane modular wrapping
 *   is enforced by masking in gather().
 * - Lane Isolation: Cross-lane carry is structurally impossible, as all arithmetic is performed
 *   on deconstructed lane values and re-packed via gather() — see this package's README for why
 *   this full unpack/repack isn't just belt-and-suspenders here, it's load-bearing.
 * - Overflow Policy: Each lane wraps/truncates modulo 2^W; the caller must ensure semantic validity
 *   (e.g., displacements stay within lane-specific bit-depth when required by higher-level policies).
 * - Semantic Validity: 'Absolute + Absolute' addition is semantically invalid at the coordinate level
 *   and should be handled by the application.
 *
 * Format Scalability:
 * - This module is width-parametric: lane widths (X_W/Y_W/Z_W) and word width (WORD_W)
 *   define the packing. Increasing these bit-field sizes does not require changes to
 *   the core operations (gather/scatter/arithmetic); only the specification constants change.
 * - Any semantic interpretation (projection, cyclic seams, clamping, carry policy) is
 *   intentionally out of scope and must be implemented by the caller.
 *
 * @module ivec
 */

import { type f64, FIELD_MASK64, type i32, type i64, type u32, type u64 } from "@xpute/core/kit/word.ts";
import type { BigTypedArray } from "@xpute/core/kit/array.ts";

import { IVEC_WORD_MASK as WORD_MASK, IVEC_X_W as X_W, IVEC_Y_W as Y_W, IVEC_Z_W as Z_W } from "./core.ts";
import { ABS, EQ, GT, LT, MAX, MIN } from "./core.ts";

/**
 * IVec: PVF3-Packed Integer Offset (Nominal Type)
 * Represents a discrete spatial unit within a multi-component grid.
 */
export type IVec = u64;

// ---- Bit Layout Configuration (Asymmetric Lane-based) ----

/**
 * SHAMT: Bit-shift alignment for X-Y-Z sequential packing.
 * Calculated internally based on the core specification to isolate lanes.
 */
const Z_SHAMT: u32 = 0;
const Y_SHAMT: u32 = Z_W;
const X_SHAMT: u32 = Z_W + Y_W;

const _Z_SHAMT: u64 = BigInt(Z_SHAMT);
const _Y_SHAMT: u64 = BigInt(Y_SHAMT);
const _X_SHAMT: u64 = BigInt(X_SHAMT);

// ---- Component Axis Isolation Masks ----

export const X_MASK: u64 = FIELD_MASK64(X_W, X_SHAMT);
export const Y_MASK: u64 = FIELD_MASK64(Y_W, Y_SHAMT);
export const Z_MASK: u64 = FIELD_MASK64(Z_W, Z_SHAMT);

/**
 * Axis Getters: Extract signed integer lanes.
 * Semantic domain follows the lane's signed two's-complement interpretation.
 */
export const X = (v: IVec): i64 => BigInt.asIntN(X_W, v >> _X_SHAMT);
export const Y = (v: IVec): i64 => BigInt.asIntN(Y_W, v >> _Y_SHAMT);
export const Z = (v: IVec): i64 => BigInt.asIntN(Z_W, v >> _Z_SHAMT);

/** Integer scalar <-> real. */
export const to_real = (comp: i64): f64 => Number(comp);
export const from_real = (real: f64): i64 => BigInt(Math.floor(real));

// ---- Logging ----

/**
 * Human-readable representation of a packed IVec index/address.
 * Purely for logging and debugging; not for runtime logic.
 */
export const repr = (v: IVec): string => `${X(v)}:${Y(v)}:${Z(v)}`;

// ---- Construction ----

/** Creates a new vector with all components initialized to a uniform value. */
export const fill = (e: i64): IVec => gather(e, e, e);

/**
 * Assembles discrete components into a packed bit-field word.
 * Strict masking (& MASK) ensures no bit leakage between component fields.
 */
export const gather = (x: i64, y: i64, z: i64): IVec => {
  const xn = (x << _X_SHAMT) & X_MASK;
  const yn = (y << _Y_SHAMT) & Y_MASK;
  const zn = (z << _Z_SHAMT) & Z_MASK;
  return xn | yn | zn;
};

/** Deconstructs a packed vector into its constituent components. */
export const scatter = (out: BigTypedArray | i64[], v: IVec): void => {
  out[0] = X(v);
  out[1] = Y(v);
  out[2] = Z(v);
};

// ---- Bitwise ----

export const not = (v: IVec): IVec => (~v) & WORD_MASK;
export const and = (v: IVec, w: IVec): IVec => v & w;
export const or = (v: IVec, w: IVec): IVec => v | w;
export const xor = (v: IVec, w: IVec): IVec => v ^ w;

/**
 * Signed shift: +k => shl, -k => shr (arith)
 * - ivec lanes are SIGNED (two's complement semantics via X/Y/Z sign-extension)
 * - right shift is arithmetic (sign-preserving) by construction of bigint >> on signed values
 */
const shl = (v: IVec, shamt: i32): IVec => {
  const _shamt: i64 = BigInt(shamt);
  return gather(X(v) << _shamt, Y(v) << _shamt, Z(v) << _shamt);
};
const shr = (v: IVec, shamt: i32): IVec => {
  const _shamt: i64 = BigInt(shamt);
  return gather(X(v) >> _shamt, Y(v) >> _shamt, Z(v) >> _shamt);
};
export const sh = (v: IVec, shamt: i32): IVec => shamt >= 0 ? shl(v, shamt) : shr(v, -shamt);

// ---- Arithmetic ----

export const neg = (v: IVec): IVec => gather(-X(v), -Y(v), -Z(v));
export const abs = (v: IVec): IVec => gather(ABS(X(v)), ABS(Y(v)), ABS(Z(v)));

/**
 * Deliberately NOT a raw `(v + w) & WORD_MASK` on the packed word — see this
 * package's README for why (three failed packed-arithmetic attempts, plus
 * why the one that does work in general isn't worth it for 3 fixed lanes on
 * a BigInt word).
 */
export const add = (v: IVec, w: IVec): IVec => gather(X(v) + X(w), Y(v) + Y(w), Z(v) + Z(w));
export const sub = (v: IVec, w: IVec): IVec => gather(X(v) - X(w), Y(v) - Y(w), Z(v) - Z(w));

export const mul = (v: IVec, w: IVec): IVec => gather(X(v) * X(w), Y(v) * Y(w), Z(v) * Z(w));
export const div = (v: IVec, w: IVec): IVec => gather(X(v) / X(w), Y(v) / Y(w), Z(v) / Z(w));
export const mod = (v: IVec, w: IVec): IVec => gather(X(v) % X(w), Y(v) % Y(w), Z(v) % Z(w));

// ---- Comparison ----

export const eq = (v: IVec, w: IVec): IVec => gather(EQ(X(v), X(w)), EQ(Y(v), Y(w)), EQ(Z(v), Z(w)));
export const equals = (v: IVec, w: IVec): boolean => v === w;
export const lt = (v: IVec, w: IVec): IVec => gather(LT(X(v), X(w)), LT(Y(v), Y(w)), LT(Z(v), Z(w)));
export const gt = (v: IVec, w: IVec): IVec => gather(GT(X(v), X(w)), GT(Y(v), Y(w)), GT(Z(v), Z(w)));
export const max_comp = (v: IVec): i64 => MAX(MAX(X(v), Y(v)), Z(v));
export const min_comp = (v: IVec): i64 => MIN(MIN(X(v), Y(v)), Z(v));
export const max = (v: IVec, w: IVec): IVec => gather(MAX(X(v), X(w)), MAX(Y(v), Y(w)), MAX(Z(v), Z(w)));
export const min = (v: IVec, w: IVec): IVec => gather(MIN(X(v), X(w)), MIN(Y(v), Y(w)), MIN(Z(v), Z(w)));
export const clamp = (v: IVec, min: i64, max: i64): IVec => gather(MIN(MAX(X(v), min), max), MIN(MAX(Y(v), min), max), MIN(MAX(Z(v), min), max));

// ---- Reduction ----

export const sum = (v: IVec): i64 => X(v) + Y(v) + Z(v);
export const dot = (v: IVec, w: IVec): i64 => sum(mul(v, w));
export const norm1 = (v: IVec): i64 => sum(abs(v));
export const norm_sq = (v: IVec): i64 => dot(v, v);
export const dist1 = (v: IVec, w: IVec): i64 => norm1(sub(v, w));
export const dist_sq = (v: IVec, w: IVec): i64 => norm_sq(sub(v, w));
