// @pxl/spec/space/pvf3/mod.ts

/**
 * [PVF3] Packed 3-Vector w/ Fixed-Point Arithmetic
 * A stateless numerical kernel designed for high-capacity spatial indexing
 * and deterministic coordinate stability. PVF3 eliminates floating-point
 * jitter by enforcing bit-level parity through BigInt-based packing.
 *
 * --- Core Modules ---
 * 1. ivec (Integer Vector):
 * - Primary spatial grid addressing (24-24-16 bit asymmetric lanes).
 * - Handles discrete unit-level positioning and global topology.
 *
 * 2. fvec (Fractional Vector):
 * - High-precision local displacement (10-10-10 bit symmetric lanes).
 * - Provides sub-grid resolution (1/1024) within a single integer block.
 *
 * 3. projection (Lattice Decomposition/Composition):
 * - Maps (x,y,z) in normalized units <-> (ivec,fvec) tuple (addr/subaddr).
 *
 * --- Operational Policy ---
 * - Deterministic: Bit-identical results across all JS environments.
 * - Stateless: Atomic operations with no side effects or internal registry.
 * - Isolated: Strict masking prevents inter-lane bit leakage during arithmetic.
 *
 * --- Structure ---
 * - Data Type : PVec (Union of IVec | FVec)
 * - Interface : PVecTraits (common packed-word arithmetic surface)
 * - Modules   : ivec, fvec (trait families)
 * - PVF3      : exported separately as a fixed-point tuple family with out-parameter arithmetic
 *
 * @module mod
 */

import type { f64, i32, i64 } from "@xpute/spec/lib/kit/word.ts";
import type { BigTypedArray } from "@xpute/spec/lib/kit/array.ts";

import type { IVec } from "./ivec.ts";
import type { FVec } from "./fvec.ts";
import * as ivec from "./ivec.ts";
import * as fvec from "./fvec.ts";

export type { IVec } from "./ivec.ts";
export type { FVec } from "./fvec.ts";
export type { PVF3 } from "./pvf3.ts";

/**
 * Packed Word Vector Union
 * Collective union of the packed-word vector families: ivec and fvec.
 */
export type PVec = IVec | FVec;

/**
 * Common operation contract implemented by the packed-word vector modules
 * (ivec and fvec).
 */
export interface PVecTraits<V extends PVec> {
  // ---- Components ---

  X(v: V): i64;
  Y(v: V): i64;
  Z(v: V): i64;

  to_real(comp: i64): f64;
  from_real(real: f64): i64;

  // ---- Logging ----

  repr(v: V): string;

  // ---- Construction ----

  fill(e: i64): V;
  gather(x: i64, y: i64, z: i64): V;
  scatter(out: BigTypedArray | i64[], v: V): void;

  // ---- Bitwise ----

  not(v: V): V; // bitwise NOT within the module's packed word width
  and(v: V, w: V): V;
  or(v: V, w: V): V;
  xor(v: V, w: V): V;

  /**
   * Signed shift operator (kernel contract).
   * - +k => left shift by k
   * - -k => right shift by k
   *
   * Semantics depend on the vector family:
   * - ivec: lanes are signed; right shift is arithmetic (sign-preserving).
   * - fvec: lanes are unsigned residues; right shift is effectively logical
   *         because extracted lanes (X/Y/Z) are non-negative (BigInt has no >>>).
   *
   * KERNEL RULE (no guards):
   * - negative or oversized shifts are caller bugs; BigInt shift may throw.
   */
  sh(v: V, shamt: i32): V;

  // ---- Arithmetic ----

  add(v: V, w: V): V;
  sub(v: V, w: V): V;
  mul(v: V, w: V): V;
  div(v: V, w: V): V;
  mod(v: V, w: V): V;

  // ---- Comparison ----

  eq(v: V, w: V): V; // packed per-lane boolean mask
  equals(v: V, w: V): boolean;
  lt(v: V, w: V): V;
  gt(v: V, w: V): V;
  max_comp(v: V): i64;
  min_comp(v: V): i64;
  max(v: V, w: V): V;
  min(v: V, w: V): V;
  clamp(v: V, min: i64, max: i64): V;

  // ---- Reduction ----

  sum(v: V): i64;
  dot(v: V, w: V): i64;
  norm1(v: V): i64;
  norm_sq(v: V): i64;
  dist1(v: V, w: V): i64;
  dist_sq(v: V, w: V): i64;
}

// Ensure modules implement the trait implicitly (Duck Typing)
// - ivec : packed signed integer lanes
// - fvec : packed unsigned fractional lanes
// - pvf3 is intentionally excluded: it is a tuple family with out-parameter arithmetic
void (ivec satisfies PVecTraits<IVec>);
void (fvec satisfies PVecTraits<FVec>);

// 3. Export Implementations
export { fvec, ivec };
export * as pvf3 from "./pvf3.ts";

export * as core from "./core.ts";
export * as projection from "./projection.ts";
