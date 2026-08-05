// @pxl/shared/spec/space/pvf3/projection.ts

/**
 * PVF3 (Packed 3-Vector w/ Fixed-Point Arithmetic)
 * Normalized Scalar <-> PVF3 Projection
 *
 * Provides generalized mapping functions for projecting continuous floating-point
 * values into hierarchical discrete layers (Integer Index + Fractional Offset)
 * and recovering them back into the scalar domain.
 *
 * Design Philosophy:
 * - Domain Agnostic: Operates on pure Euclidean scalar space. Makes no assumptions
 *   about coordinate systems or physical units.
 * - Hierarchical Consistency: Preserves lattice continuity across negative domains.
 * - Mathematical Determinism: Bridges IEEE 754 continuity and BigInt-packed precision.
 *
 * TERMINOLOGY
 * -----------
 * This module operates in a *normalized scalar space*:
 * - 1.0 means "one lattice unit" (caller defines what that unit is).
 * - Scaling to/from meters (or any physical unit) is done outside.
 *
 * IDENTITY
 * --------
 * This module performs a basis transpose between:
 * - axis-paired form : [[x_i, x_f], [y_i, y_f], [z_i, z_f]]
 * - lane-packed form : [[x_i, y_i, z_i], [x_f, y_f, z_f]] == { i, f }
 *
 * In other words, it factors a scalar vector into:
 * - integer lattice lanes (IVec) and
 * - fractional remainder lanes (FVec),
 * and packs them into PVF3.
 *
 * @module projection
 */

import type { f64, i64, u32 } from "@xpute/core/kit/word.ts";

import type { FVec, IVec, PVF3 } from "./mod.ts";
import { fvec, ivec } from "./mod.ts";
import type { V3 } from "../v3.ts";

// [x_i, x_f, y_i, y_f, z_i, z_f]
const REG = new BigInt64Array(6);

/**
 * Scalar projection.
 *
 * Decomposes one normalized scalar into:
 * - i : integer lattice lane
 * - f : fractional residue lane
 *
 * Notes:
 * - Uses floor() to preserve continuity across negative domains.
 *   Example: -0.2 => i = -1, f = 0.8
 */
const project_comp = (axis: u32, comp: f64): void => {
  // i_comp: integer lattice position (stable across negative domain)
  REG[2 * axis] = ivec.from_real(comp);
  // f_comp: map fractional remainder into fixed-point lane resolution
  REG[2 * axis + 1] = fvec.from_real(comp);
};

/**
 * Vectorized projection.
 *
 * Converts (x,y,z) in normalized lattice units into PVF3:
 * - i : packed integer lanes
 * - f : packed fractional residue lanes
 */
export const project = (out: PVF3, x: f64, y: f64, z: f64): PVF3 => {
  project_comp(0, x);
  project_comp(1, y);
  project_comp(2, z);
  out.i = ivec.gather(REG[0], REG[2], REG[4]);
  out.f = fvec.gather(REG[1], REG[3], REG[5]);
  return out;
};

/**
 * Reconstructs a normalized scalar from:
 * - i_comp : integer lattice index
 * - f_comp : fractional remainder (fixed-point lane)
 *
 * @returns Normalized coordinate in "lattice units".
 */
const unproject_comp = (i_comp: i64, f_comp: i64): f64 => {
  return ivec.to_real(i_comp) + fvec.to_real(f_comp);
};

/**
 * Vectorized unprojection.
 *
 * Writes reconstructed normalized (x,y,z) into out[0..2].
 * - out is intentionally "array-like" to match gl-matrix style.
 */
export const unproject = (out: V3, i: IVec, f: FVec): V3 => {
  out[0] = unproject_comp(ivec.X(i), fvec.X(f));
  out[1] = unproject_comp(ivec.Y(i), fvec.Y(f));
  out[2] = unproject_comp(ivec.Z(i), fvec.Z(f));
  return out;
};
