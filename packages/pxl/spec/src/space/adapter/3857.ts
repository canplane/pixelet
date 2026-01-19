// @pxl/spec/space/adapter/3857.ts

/**
 * EPSG:3857 domain adapter
 *
 * ROLE
 * ----
 * Bridges between:
 * - Physical space : EPSG:3857 planar meters
 * - Canonical space: G64 coordinates expressed in internal `u` and stored in PVF3
 *
 * NOTES
 * -----
 * - PVF3 kernels operate on canonical internal units (`u`), not meters.
 * - In the current mapping, 1u = 1 L1 cell.
 * - In this adapter, `m` always means an EPSG:3857 planar meter.
 * - EPSG:3857 scale is latitude-dependent:
 *   - at the equator, 1 planar meter matches ~1 real meter
 *   - away from the equator, local ground scale is non-linear and distorted
 * - Physical scaling, Mercator wrapping, and vertical correction are handled
 *   exclusively in this adapter layer.
 *
 * @module epsg3857
 */

import { type f64, type u32 } from "@xpute/spec/lib/kit/word.ts";

import { type FVec, type IVec, projection, type PVF3 } from "../pvf3/mod.ts";
import { IVEC_X_W } from "../pvf3/core.ts";
import { FROM_L3_IDX, L3_LIMIT, L3_RANGE } from "../indexer.ts";
import type { V3 } from "../v3.ts";

import * as epsg4326 from "./4326.ts";

/**
 * EPSG:3857 absolute coordinates (planar meters).
 */
export type G3857 = [
  x: f64,
  y: f64,
  z?: f64,
];

// World width at the equator in EPSG:3857 planar meters
const WORLD_WIDTH_3857: f64 = 40075016.68557849;
const HALF_W_3857: f64 = WORLD_WIDTH_3857 / 2;

const V3_REG: V3 = [0, 0, 0];

/**
 * Canonical spatial units per physical meter in the EPSG:3857 adapter plane.
 *
 * Unit model:
 * - `u` is the canonical internal spatial unit.
 * - In the current mapping: 1u = 1 L1 cell.
 * - `m` means an EPSG:3857 planar meter, not a geodesic ground meter.
 *
 * 🌍 [Cyclic World Topology Mapping]
 * 24비트 정수 공간(2^24 = 16,777,216 u)이 지구 적도 둘레(WORLD_WIDTH_3857)와
 * 1:1로 정확하게 일치하도록 스케일을 동적으로 락온(Lock-on)합니다.
 * * - 1 [u] = 약 2.388 [m]
 * - 24-bit MAX (8,388,607) 지점을 넘어서면 하드웨어 레벨에서 음수(-8,388,608)로 오버플로우됩니다.
 * - 이 오버플로우는 실제 지구의 날짜변경선(Antimeridian)을 건너는 것과 물리적으로 완벽히 동일합니다.
 *
 * Note:
 * - Because EPSG:3857 is distorted, these meters match real-world meters only
 * at the equator; higher latitudes require Mercator scale correction.
 */
const U_TOTAL_X: u32 = 1 << IVEC_X_W; // 16,777,216

export const U_PER_M: f64 = U_TOTAL_X / WORLD_WIDTH_3857; // 1 [m] = 약 0.418 [u]
const M_PER_U: f64 = WORLD_WIDTH_3857 / U_TOTAL_X; // 1 [u] = 약 2.388 [m]

/**
 * Public Mercator vertical scale contract.
 *
 * Semantics
 * ---------
 * - Input `y_m` is an EPSG:3857 planar Y coordinate in meters.
 * - The returned scale is NOT evaluated at the exact input point.
 * - Instead, the input is first snapped to the owning L3 page base in canonical `u`,
 *   then converted back to EPSG:3857 meters, and only that page-base Y is used.
 *
 * Why coarse?
 * -----------
 * - This project intentionally uses a page-constant Mercator Z scale.
 * - All world/page/object systems must share the same coarse rule so that:
 *   - terrain,
 *   - feature/object heights,
 *   - page baking,
 *   - page-local rendering
 *   do not drift from each other by mixing slightly different per-point scales.
 *
 * Contract
 * --------
 * - Within the same L3 page, all inputs return the same scale.
 * - Scale changes only when crossing an L3 page boundary in Y.
 * - Fine-grained pointwise Mercator scale is kept internal only and must not be
 *   used as a public world/render rule.
 */
const Z_SCALE_STRIDE_U: f64 = Number(L3_RANGE);

const Z_SCALE_FINE = (y_m: f64): f64 => {
  const lat_rad = (epsg4326.Y_TO_LAT(y_m) * Math.PI) / 180;
  return 1 / Math.cos(lat_rad);
};

export const Z_SCALE = (y_m: f64): f64 => {
  const y_u = y_m * U_PER_M;
  const y_base_u = Math.floor(y_u / Z_SCALE_STRIDE_U) * Z_SCALE_STRIDE_U;
  const y_base_m = y_base_u * M_PER_U;
  return Z_SCALE_FINE(y_base_m);
};

/**
 * EPSG:3857 planar meters -> G64 coordinates (stored in PVF3)
 *
 * PIPELINE
 * --------
 * 1) Normalize X into the canonical Mercator range (wrap-around seam)
 * 2) Apply Mercator vertical scaling to altitude
 * 3) Convert meters to normalized lattice units
 * 4) Store the projected G64 coordinate into PVF3
 */
export const project = (out: PVF3, x: f64, y: f64, z: f64 = 0): PVF3 => {
  // Canonical Mercator wrap-around
  x = ((((x + HALF_W_3857) % WORLD_WIDTH_3857) + WORLD_WIDTH_3857) % WORLD_WIDTH_3857) - HALF_W_3857;
  // Vertical Mercator correction
  z = z * Z_SCALE(y);

  // Physical meters -> normalized lattice units
  x *= U_PER_M;
  y *= U_PER_M;
  z *= U_PER_M;
  projection.project(out, x, y, z);

  return out;
};

/**
 * G64 coordinates (stored in PVF3) -> EPSG:3857 planar meters
 */
export const unproject = (out: G3857, i: IVec, f: FVec): G3857 => {
  projection.unproject(V3_REG, i, f);

  // Normalized units -> physical meters
  const x: f64 = V3_REG[0] * M_PER_U;
  const y: f64 = V3_REG[1] * M_PER_U;
  const z: f64 = V3_REG[2] * M_PER_U;

  // Canonical Mercator wrap-around
  out[0] = ((((x + HALF_W_3857) % WORLD_WIDTH_3857) + WORLD_WIDTH_3857) % WORLD_WIDTH_3857) - HALF_W_3857;
  out[1] = y;
  // Undo vertical correction
  out[2] = z / Z_SCALE(y);

  return out;
};

// ---- Bounding Box ----

export type BBox3857 = [min: G3857, max: G3857];

export function get_page_bbox(page_idx: IVec): BBox3857 {
  const base: IVec = FROM_L3_IDX(page_idx);
  const limit: IVec = L3_LIMIT(base);

  const min: G3857 = [0, 0, 0];
  const max: G3857 = [0, 0, 0];
  unproject(min, base, 0n);
  unproject(max, limit, 0n);

  return [min, max];
}
