// @pxl/web/render/layer/terrain/elevation.ts

/**
 * Coordinate prefix rules:
 * - c (chunk)  : chunk lattice index (e.g. cx, cy)
 * - d (delta)  : local loop delta (e.g. dx, dy)
 * - t (texel)  : continuous tile texel / pixel coordinates (e.g. tx, ty)
 * - s (source) : source-grid coordinates in the padded low-res array (e.g. sx, sy)
 */

import type { f64, i16, i32, u32 } from "@xpute/core/kit/word.ts";
import type { I16Array } from "@xpute/core/kit/array.ts";
import { clamp, lerp } from "@xpute/core/kit/math.ts";

import { CHUNK_CELL_SPAN, ELE_PAD, ELE_PADDED_SPAN, PAGE_CELL_SPAN, PAGE_ELE_SPAN } from "@pxl/shared/bake/constant.ts";
import type { Ground } from "@pxl/shared/bake/format.ts";
import { cube_types } from "@pxl/shared/spec/cell/cube.ts";

const ELE_CELL_STEP: f64 = PAGE_CELL_SPAN / PAGE_ELE_SPAN;

/**
 * Unit convention:
 * - [u]      : Canonical world unit (Standard coordinate space).
 * - [cell]   : Discrete raster unit. 1u = 2^-CELL_SHAMT cells.
 * - [texel]  : Continuous coordinate within the PAGE_CELL_SPAN grid [0, PAGE_CELL_SPAN].
 * - [sample] : Low-res elevation grid index [0, 32] (before padding).
 *
 * Scale relation:
 * - 1 [cell] = 1 [texel]
 * - 1 [u]    = 2^-CELL_SHAMT [cells] (currently CELL_SHAMT=-1, so 1u = 2 cells)
 * - Elevation stored in I16Array is in [cell] units to preserve 0.25u precision.
 */

/**
 * Bilinear elevation sample from the padded 34x34 elevation tile raster.
 *
 * Coordinate convention:
 * - (tx, ty) are floating-point continuous texel coordinates.
 * - valid tile-texel domain is typically [0, PAGE_CELL_SPAN]
 * - source samples are center samples at:
 *     (i - pad + 0.5) * step
 *
 * Therefore the source-grid coordinates are:
 *   sx = tx / step + pad - 0.5
 *   sy = ty / step + pad - 0.5
 */
function _sample_ele_bilinear(page_ele: I16Array, tx: u32, ty: u32): i16 {
  // convert texel-space coordinates into padded source-grid coordinates.
  const sx: f64 = (tx + 0.5) / ELE_CELL_STEP + ELE_PAD - 0.5;
  const sy: f64 = (ty + 0.5) / ELE_CELL_STEP + ELE_PAD - 0.5;

  const sx0: u32 = clamp(Math.floor(sx), 0, ELE_PADDED_SPAN - 1);
  const sy0: u32 = clamp(Math.floor(sy), 0, ELE_PADDED_SPAN - 1);
  const sx1: u32 = clamp(sx0 + 1, 0, ELE_PADDED_SPAN - 1);
  const sy1: u32 = clamp(sy0 + 1, 0, ELE_PADDED_SPAN - 1);

  const sdx: f64 = sx - sx0;
  const sdy: f64 = sy - sy0;

  // bilinear interpolation
  const a: i16 = page_ele[sy0 * ELE_PADDED_SPAN + sx0];
  const b: i16 = page_ele[sy0 * ELE_PADDED_SPAN + sx1];
  const c: i16 = page_ele[sy1 * ELE_PADDED_SPAN + sx0];
  const d: i16 = page_ele[sy1 * ELE_PADDED_SPAN + sx1];

  const ab: f64 = lerp(a, b, sdx);
  const cd: f64 = lerp(c, d, sdx);
  const out: f64 = lerp(ab, cd, sdy);

  return Math.round(out);
}

/**
 * Bicubic elevation sampling from the padded source raster.
 *
 * Notes:
 * - keeps the hot path allocation-free
 * - uses an unclamped fast path for the common interior case
 * - falls back to clamped sampling near tile boundaries
 */
function sample_ele_bicubic(page_ele: I16Array, tx: u32, ty: u32): i16 {
  const sx: f64 = (tx + 0.5) / ELE_CELL_STEP + ELE_PAD - 0.5;
  const sy: f64 = (ty + 0.5) / ELE_CELL_STEP + ELE_PAD - 0.5;

  const sx0: i32 = Math.floor(sx);
  const sy0: i32 = Math.floor(sy);

  const fdx: f64 = sx - sx0;
  const fdy: f64 = sy - sy0;

  // precompute cubic interpolation weights.
  const x2 = fdx * fdx;
  const x3 = x2 * fdx;
  const wx0 = -0.5 * x3 + x2 - 0.5 * fdx;
  const wx1 = 1.5 * x3 - 2.5 * x2 + 1.0;
  const wx2 = -1.5 * x3 + 2.0 * x2 + 0.5 * fdx;
  const wx3 = 0.5 * x3 - 0.5 * x2;

  const y2 = fdy * fdy;
  const y3 = y2 * fdy;
  const wy0 = -0.5 * y3 + y2 - 0.5 * fdy;
  const wy1 = 1.5 * y3 - 2.5 * y2 + 1.0;
  const wy2 = -1.5 * y3 + 2.0 * y2 + 0.5 * fdy;
  const wy3 = 0.5 * y3 - 0.5 * y2;

  const span = ELE_PADDED_SPAN;

  // fast interior path with no clamp overhead.
  if (sx0 >= 1 && sx0 <= span - 3 && sy0 >= 1 && sy0 <= span - 3) {
    // unrolled 4x4 neighborhood sampling.
    const idx0 = (sy0 - 1) * span + sx0;
    const idx1 = sy0 * span + sx0;
    const idx2 = (sy0 + 1) * span + sx0;
    const idx3 = (sy0 + 2) * span + sx0;

    const row0 = page_ele[idx0 - 1] * wx0 + page_ele[idx0] * wx1 + page_ele[idx0 + 1] * wx2 + page_ele[idx0 + 2] * wx3;
    const row1 = page_ele[idx1 - 1] * wx0 + page_ele[idx1] * wx1 + page_ele[idx1 + 1] * wx2 + page_ele[idx1 + 2] * wx3;
    const row2 = page_ele[idx2 - 1] * wx0 + page_ele[idx2] * wx1 + page_ele[idx2 + 1] * wx2 + page_ele[idx2 + 2] * wx3;
    const row3 = page_ele[idx3 - 1] * wx0 + page_ele[idx3] * wx1 + page_ele[idx3 + 1] * wx2 + page_ele[idx3 + 2] * wx3;

    return Math.round(row0 * wy0 + row1 * wy1 + row2 * wy2 + row3 * wy3);
  }

  // slow boundary path with clamped coordinates.
  let out: f64 = 0;
  const wx = [wx0, wx1, wx2, wx3];
  const wy = [wy0, wy1, wy2, wy3];

  for (let j = 0; j < 4; j++) {
    const py = clamp(sy0 - 1 + j, 0, span - 1);
    let row_out: f64 = 0;
    for (let i = 0; i < 4; i++) {
      const px = clamp(sx0 - 1 + i, 0, span - 1);
      row_out += page_ele[py * span + px] * wx[i];
    }
    out += row_out * wy[j];
  }

  return Math.round(out);
}

export const sample_ele = sample_ele_bicubic;

/** How much real ground elevation drops for water-look terrain (RIVER/LAKE/
 * WETLAND, and SEA — its own land type, not the WATER channel, see
 * is_water_family_at) — a real height change, not a cosmetic mesh-only
 * offset: it's applied here, at the one shared elevation query surface, so
 * every consumer (chunk mesh, placement/snap raycast — both read this
 * through chunk.ts's slice_chunk — and module/world/terrain/elevation.ts's
 * own camera ground-snap, which reads a page's raw ele directly) sees the
 * same lowered height instead of only the rendered mesh looking recessed
 * while placement math, ground-snap, or a cell marker still reasons about
 * the un-recessed one. */
export const WATER_RECESS_CELLS: i16 = 1;

/** Mirrors render/layer/terrain/adapter.ts's ground_material_at priority
 * (LAND < WATER < STRUCTURE < TRANSPORT, last non-VOID wins) enough to
 * answer one question: does this cell look like open water? Kept
 * independent rather than imported from adapter.ts, which pulls in
 * Zustand store bindings this pure math module has no business depending
 * on — if adapter.ts's channel priority ever changes, this needs the same
 * change made here too. SEA is checked separately from the WATER channel:
 * cube.ts models it as land.SEA (a masked BARE-family id), not one of
 * WATER's own RIVER/LAKE/WETLAND types. */
export function is_water_family_at(ground: Ground, idx: u32): boolean {
  if (ground.structure[idx] !== cube_types.common.VOID) return false;
  if (ground.transport[idx] !== cube_types.common.VOID) return false;
  if (ground.water[idx] !== cube_types.common.VOID) return true;
  return ground.land[idx] === cube_types.land.SEA;
}

/**
 * Unpacks a dense (CHUNK_CELL_SPAN x CHUNK_CELL_SPAN) elevation chunk from the coarse heightmap.
 * Samples are taken at cell centers (0.5 offset) to match DB-side resampling.
 */
export function dense_ele(page_ele: I16Array, tx0: u32, ty0: u32, ground?: Ground): I16Array {
  return dense_ele_region(page_ele, tx0, ty0, CHUNK_CELL_SPAN, ground);
}

/** Dense reconstruction for a renderer-owned square region. `ground`, when
 * given, must already be sliced/aligned to this same tx0/ty0/span region
 * (see chunk.ts's slice_chunk) — every water-family cell in it gets
 * WATER_RECESS_CELLS subtracted from its sampled height. */
export function dense_ele_region(page_ele: I16Array, tx0: u32, ty0: u32, span: u32, ground?: Ground, out: I16Array = new Int16Array(span * span)): I16Array {
  let idx: u32 = 0;
  for (let dy: u32 = 0; dy < span; dy++) {
    const ty: u32 = ty0 + dy;
    for (let dx: u32 = 0; dx < span; dx++) {
      const tx: u32 = tx0 + dx;
      let ele = sample_ele(page_ele, tx, ty);
      if (ground && is_water_family_at(ground, idx)) ele -= WATER_RECESS_CELLS;
      out[idx++] = ele;
    }
  }

  return out;
}
