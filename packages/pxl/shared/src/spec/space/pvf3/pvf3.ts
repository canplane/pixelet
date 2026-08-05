// @pxl/shared/spec/space/pvf3/pvf3.ts

/**
 * PVF3 (Packed 3-Vector w/ Fixed-Point Arithmetic)
 * Fixed-Point Scalar System
 *
 * This module provides atomic, stateless operations for fixed-point arithmetic
 * over PVF3 = [IVec, FVec], where each axis is interpreted as a signed packed
 * fixed-point scalar composed of:
 * - ivec lane: signed integer part
 * - fvec lane: unsigned fractional residue
 *
 * Memory Layout (Per-Axis Fixed-Point Scalar):
 * [ signed integer part | unsigned fractional residue (FRAC_W bits) ]
 *
 * Example Interpretation:
 * - X(v) = ivec.X(v.i) * 2^FRAC_W + fvec.X(v.f)
 * - Y(v) = ivec.Y(v.i) * 2^FRAC_W + fvec.Y(v.f)
 * - Z(v) = ivec.Z(v.i) * 2^FRAC_W + fvec.Z(v.f)
 *
 * Operational Integrity & Overflow Policy:
 * - Deterministic Arithmetic: Uses BigInt for bit-exact fixed-point operations.
 * - Lane Isolation: Runtime storage remains separated into ivec/fvec; arithmetic
 *   is performed on reconstructed fixed-point scalars and re-packed.
 * - Overflow Policy: Final truncation/wrapping is delegated to ivec.gather and
 *   fvec.gather when results are written back into PVF3 storage.
 * - Division Policy: Uses the same bigint integer division/modulo semantics as
 *   current ivec/fvec (i.e. JS bigint `/`, `%`).
 *
 * Format Scalability:
 * - Fractional precision is width-parametric via FRAC_W.
 * - Increasing fractional width does not require changes to the arithmetic
 *   surface; only the specification constants change.
 *
 * @module pvf3
 */

import type { f64, i32, i64, u64 } from "@xpute/core/kit/word.ts";
import type { BigTypedArray } from "@xpute/core/kit/array.ts";

import { ABS, EQ, FRAC_W, GT, LT, MAX, MIN, ONE, ONE_MASK } from "./core.ts";
import type { IVec } from "./ivec.ts";
import type { FVec } from "./fvec.ts";
import * as ivec from "./ivec.ts";
import * as fvec from "./fvec.ts";

/**
 * PVF3: Consolidated 96-bit Address Tuple
 *
 * Represents a point in a 3D lattice space using:
 * - i : packed signed integer lanes (coarse lattice index)
 * - f : packed unsigned fixed-point lanes (local remainder within a cell)
 *
 * Notes:
 * - This type is unitless. Physical meaning (meters, tiles, voxels, etc.)
 *   is defined entirely by the caller through scaling outside the kernel.
 * - Default configuration provides fractional resolution of 2^-FRAC_W
 *   per axis (e.g. FRAC_W=10 -> 1024 steps per lattice unit).
 *
 * @property i - 64-bit packed integer address (coarse lattice).
 * @property f - 32-bit packed fractional remainder (fixed-point lanes).
 */
export type PVF3 = { i: IVec; f: FVec };

/**
 * Shared scratch register for PVF3 temporary reductions.
 *
 * WARNING:
 * - mutable module-level scratch
 * - synchronous / non-reentrant only
 * - do not use across await, callbacks, or reentrant boundaries
 * - contents are volatile and may be overwritten by any helper using this register
 */
const PVF3_REG: PVF3 = { i: 0n, f: 0n };

const _FRAC_W: u64 = BigInt(FRAC_W);

// ---- Bit Layout Configuration (Fixed-Point Scalar-based) ----

/**
 * Axis Getters: Reconstruct signed fixed-point scalars from ivec/fvec storage.
 * Returned as i64 fixed-point scalars with FRAC_W fractional bits.
 */
export const X = (v: PVF3): i64 => (ivec.X(v.i) << _FRAC_W) | fvec.X(v.f);
export const Y = (v: PVF3): i64 => (ivec.Y(v.i) << _FRAC_W) | fvec.Y(v.f);
export const Z = (v: PVF3): i64 => (ivec.Z(v.i) << _FRAC_W) | fvec.Z(v.f);

/** Fixed-point scalar <-> real. */
export const to_real = (comp: i64): f64 => Number(comp) / Number(ONE);
export const from_real = (real: f64): i64 => BigInt(Math.floor(Number(ONE) * real));

// ---- Logging ----

/**
 * Human-readable representation of packed fixed-point axis scalars.
 * Purely for logging and debugging; not for runtime logic.
 */
export const repr = (v: PVF3): string => `{ i: (${ivec.repr(v.i)}), f: (${fvec.repr(v.f)}) }`;

// ---- Construction ----

/** Writes the same fixed-point scalar into all three axes. */
export const fill = (out: PVF3, e: i64): PVF3 => gather(out, e, e, e);

/**
 * Gather from fixed-point packed scalars.
 *
 * Takes per-axis fixed-point scalars and re-packs them into [IVec, FVec]:
 * - integer part   = arithmetic right shift by FRAC_W
 * - fractional part = low FRAC_W bits
 */
export const gather = (out: PVF3, x: i64, y: i64, z: i64): PVF3 => {
  out.i = ivec.gather(x >> _FRAC_W, y >> _FRAC_W, z >> _FRAC_W);
  out.f = fvec.gather(x & ONE_MASK, y & ONE_MASK, z & ONE_MASK);
  return out;
};

/** Deconstructs a packed vector into its constituent fixed-point scalars. */
export const scatter = (out: BigTypedArray | i64[], v: PVF3): void => {
  out[0] = X(v);
  out[1] = Y(v);
  out[2] = Z(v);
};

// ---- Bitwise ----

/**
 * Reuse raw storage-domain bitwise ops directly.
 *
 * This is the closest extension of ivec/fvec behavior:
 * - ivec bitwise on integer part
 * - fvec bitwise on fractional part
 */
export const not = (out: PVF3, v: PVF3): PVF3 => {
  out.i = ivec.not(v.i);
  out.f = fvec.not(v.f);
  return out;
};
export const and = (out: PVF3, v: PVF3, w: PVF3): PVF3 => {
  out.i = ivec.and(v.i, w.i);
  out.f = fvec.and(v.f, w.f);
  return out;
};
export const or = (out: PVF3, v: PVF3, w: PVF3): PVF3 => {
  out.i = ivec.or(v.i, w.i);
  out.f = fvec.or(v.f, w.f);
  return out;
};
export const xor = (out: PVF3, v: PVF3, w: PVF3): PVF3 => {
  out.i = ivec.xor(v.i, w.i);
  out.f = fvec.xor(v.f, w.f);
  return out;
};

/**
 * Signed shift: +k => shl, -k => shr (arith)
 * - pvf3 axes are SIGNED fixed-point scalars reconstructed by X/Y/Z
 * - right shift is arithmetic (sign-preserving) by construction of bigint >> on signed values
 */
const shl = (out: PVF3, v: PVF3, shamt: i32): PVF3 => {
  const _shamt: i64 = BigInt(shamt);
  return gather(out, X(v) << _shamt, Y(v) << _shamt, Z(v) << _shamt);
};
const shr = (out: PVF3, v: PVF3, shamt: i32): PVF3 => {
  const _shamt: i64 = BigInt(shamt);
  return gather(out, X(v) >> _shamt, Y(v) >> _shamt, Z(v) >> _shamt);
};
export const sh = (out: PVF3, v: PVF3, shamt: i32): PVF3 => shamt >= 0 ? shl(out, v, shamt) : shr(out, v, -shamt);

// ---- Arithmetic ----

/**
 * Arithmetic is defined per-axis on reconstructed fixed-point scalars:
 *   op(v, w) = gather(op(X(v), X(w)), op(Y(v), Y(w)), op(Z(v), Z(w)))
 */
export const neg = (out: PVF3, v: PVF3): PVF3 => gather(out, -X(v), -Y(v), -Z(v));
export const abs = (out: PVF3, v: PVF3): PVF3 => gather(out, ABS(X(v)), ABS(Y(v)), ABS(Z(v)));
export const add = (out: PVF3, v: PVF3, w: PVF3): PVF3 => gather(out, X(v) + X(w), Y(v) + Y(w), Z(v) + Z(w));
export const sub = (out: PVF3, v: PVF3, w: PVF3): PVF3 => gather(out, X(v) - X(w), Y(v) - Y(w), Z(v) - Z(w));

/**
 * Fixed-point multiply.
 * X/Y/Z already carry FRAC_W fractional bits, so the product is rescaled once by ONE.
 */
export const mul = (out: PVF3, v: PVF3, w: PVF3): PVF3 => gather(out, (X(v) * X(w)) / ONE, (Y(v) * Y(w)) / ONE, (Z(v) * Z(w)) / ONE);

/**
 * Fixed-point divide.
 * To preserve FRAC_W fractional bits, the numerator is pre-scaled by ONE.
 */
export const div = (out: PVF3, v: PVF3, w: PVF3): PVF3 => gather(out, (X(v) << _FRAC_W) / X(w), (Y(v) << _FRAC_W) / Y(w), (Z(v) << _FRAC_W) / Z(w));

/** Fixed-point remainder in the packed scalar domain. */
export const mod = (out: PVF3, v: PVF3, w: PVF3): PVF3 => gather(out, X(v) % X(w), Y(v) % Y(w), Z(v) % Z(w));

// ---- Comparison ----

export const eq = (out: PVF3, v: PVF3, w: PVF3): PVF3 => gather(out, EQ(X(v), X(w)), EQ(Y(v), Y(w)), EQ(Z(v), Z(w)));
export const equals = (v: PVF3, w: PVF3): boolean => v.i === w.i && v.f === w.f;
export const lt = (out: PVF3, v: PVF3, w: PVF3): PVF3 => gather(out, LT(X(v), X(w)), LT(Y(v), Y(w)), LT(Z(v), Z(w)));
export const gt = (out: PVF3, v: PVF3, w: PVF3): PVF3 => gather(out, GT(X(v), X(w)), GT(Y(v), Y(w)), GT(Z(v), Z(w)));
export const max_comp = (v: PVF3): i64 => MAX(MAX(X(v), Y(v)), Z(v));
export const min_comp = (v: PVF3): i64 => MIN(MIN(X(v), Y(v)), Z(v));
export const max = (out: PVF3, v: PVF3, w: PVF3): PVF3 => gather(out, MAX(X(v), X(w)), MAX(Y(v), Y(w)), MAX(Z(v), Z(w)));
export const min = (out: PVF3, v: PVF3, w: PVF3): PVF3 => gather(out, MIN(X(v), X(w)), MIN(Y(v), Y(w)), MIN(Z(v), Z(w)));
export const clamp = (out: PVF3, v: PVF3, minv: i64, maxv: i64): PVF3 => gather(out, MIN(MAX(X(v), minv), maxv), MIN(MAX(Y(v), minv), maxv), MIN(MAX(Z(v), minv), maxv));

// ---- Reduction ----

export const sum = (v: PVF3): i64 => X(v) + Y(v) + Z(v);
export const dot = (v: PVF3, w: PVF3): i64 => sum(mul(PVF3_REG, v, w));
export const norm1 = (v: PVF3): i64 => sum(abs(PVF3_REG, v));
export const norm_sq = (v: PVF3): i64 => dot(v, v);
export const dist1 = (v: PVF3, w: PVF3): i64 => norm1(sub(PVF3_REG, v, w));
export const dist_sq = (v: PVF3, w: PVF3): i64 => norm_sq(sub(PVF3_REG, v, w));
