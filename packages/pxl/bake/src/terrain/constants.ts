// @pxl/bake/terrain/constants.ts

import type { f64, i32, u32 } from "@xpute/spec/lib/kit/word.ts";

import { L2_SHAMT, L3_SHAMT } from "@pxl/spec/space/indexer.ts";
import { U_PER_M, Z_SCALE } from "@pxl/spec/space/adapter/3857.ts";

// ---- Unit System ----

/**
 * UNIT DEFINITIONS:
 * - [u]    : Canonical world unit.
 * - [cell] : Discrete raster unit.
 * - [m]    : Physical meters (EPSG:3857 based).
 *
 * SCALE HIERARCHY:
 * - CELL_SHAMT defines the cell<->u relation.
 * - when CELL_SHAMT = 0, 1u = 1cell.
 */
export const CELL_SHAMT: i32 = -1;
export const U_PER_CELL: f64 = 2 ** CELL_SHAMT;
// Valid when CELL_SHAMT <= 0.
const CELL_PER_U: u32 = 1 << -CELL_SHAMT;

export const XY_CELL_PER_M: f64 = CELL_PER_U * U_PER_M;

/**
 * Vertical [cell] units per physical meter at the owning L3 page.
 *
 * Semantics:
 * - Input `y_m` is EPSG:3857 planar Y in meters.
 * - Mercator vertical scale is snapped to the owning L3 page base via `Z_SCALE(y_m)`.
 * - All points within the same L3 page therefore share the same Z quantization factor.
 *
 * Used for DB/bake-side elevation and feature height quantization so that
 * terrain/object Z contracts match the shared page-constant Mercator rule.
 */
export const Z_CELL_PER_M = (y_m: f64): f64 => XY_CELL_PER_M * Z_SCALE(y_m);

// ---- Grid Layout (Horizontal) ----

/** One L3 Page span in raster cells. */
export const PAGE_CELL_SPAN: u32 = 1 << (L3_SHAMT - CELL_SHAMT);

/** One L2 Chunk span in raster cells. */
export const CHUNK_CELL_SPAN: u32 = 1 << (L2_SHAMT - CELL_SHAMT);

// ---- Elevation (Sampling Contract) ----

/**
 * ELEVATION SAMPLING CONTRACT:
 *
 * 1. CENTER-SAMPLE CONVENTION:
 *    Both DB-side resampling (SQL) and Client-side reconstruction (Bicubic)
 *    MUST use cell-center coordinates (x + 0.5, y + 0.5).
 *
 * 2. INTERPOLATION MAPPING:
 *    Source grid coordinates [sx, sy] are derived from texel [tx, ty] via:
 *    step = PAGE_CELL_SPAN / PAGE_ELE_SPAN (e.g., 32)
 *    sx = tx / step + pad - 0.5
 *
 * 3. QUANTIZATION:
 *    Elevation is stored as I16 [cell] units.
 *    Renderer must multiply by U_PER_CELL to restore world [u] units.
 */

/** Number of elevation samples across one page (32x32 inner grid). */
export const PAGE_ELE_SPAN: u32 = 1 << (L3_SHAMT + 1 - 5);

/**
 * Two-cell padding (ELE_PAD=2) enables C1 continuous Bicubic interpolation
 * at page boundaries by providing a 4x4 sample kernel for every edge texel.
 * (32 inner + 2*2 padding = 36x36 total grid).
 */
export const ELE_PAD: u32 = 2;
export const ELE_PADDED_SPAN: u32 = PAGE_ELE_SPAN + 2 * ELE_PAD;

// ---- Sparse Tree ----

export const TERRAIN_LAYOUT = {
  dim: 2,

  min: CELL_SHAMT,
  max: L3_SHAMT,
} as const;
