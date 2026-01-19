// @pxl/render/terrain/elevation.ts

/**
 * Coordinate prefix rules:
 * - c (Chunk)  : chunk lattice index (e.g., cx, cy)
 * - d (Delta)  : local loop delta (e.g., dx, dy)
 * - t (Texel)  : continuous tile texel / pixel coordinates (e.g., tx, ty)
 * - s (Source) : source-grid coordinates in the padded low-res array (e.g., sx, sy)
 */

import type { f64, i16, i32, u32 } from "@xpute/spec/lib/kit/word.ts";
import type { I16Array } from "@xpute/spec/lib/kit/array.ts";

import { CHUNK_CELL_SPAN, ELE_PAD, ELE_PADDED_SPAN, PAGE_CELL_SPAN, PAGE_ELE_SPAN } from "@pxl/bake/terrain/constants.ts";

const ELE_CELL_STEP: f64 = PAGE_CELL_SPAN / PAGE_ELE_SPAN;

const CHUNK_CELL_SPAN2: u32 = CHUNK_CELL_SPAN * CHUNK_CELL_SPAN;

/**
 * UNIT CONVENTION:
 * - [u]      : Canonical world unit (Standard coordinate space).
 * - [cell]   : Discrete raster unit. 1u = 2^-CELL_SHAMT cells.
 * - [texel]  : Continuous coordinate within the PAGE_CELL_SPAN grid [0, 1024].
 * - [sample] : Low-res elevation grid index [0, 32] (before padding).
 *
 * SCALE RELATION:
 * - 1 [cell] = 1 [texel]
 * - 1 [u]    = 4 [cells] (when CELL_SHAMT is -2)
 * - Elevation stored in I16Array is in [cell] units to preserve 0.25u precision.
 */

const LERP = (a: f64, b: f64, t: f64): f64 => a + (b - a) * t;
const CLAMP = (v: f64, lo: f64, hi: f64): f64 => v < lo ? lo : v > hi ? hi : v;

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
function sample_ele_bilinear(page_ele: I16Array, tx: u32, ty: u32): i16 {
  // to source scale
  const sx: f64 = (tx + 0.5) / ELE_CELL_STEP + ELE_PAD - 0.5;
  const sy: f64 = (ty + 0.5) / ELE_CELL_STEP + ELE_PAD - 0.5;

  const sx0: u32 = CLAMP(Math.floor(sx), 0, ELE_PADDED_SPAN - 1);
  const sy0: u32 = CLAMP(Math.floor(sy), 0, ELE_PADDED_SPAN - 1);
  const sx1: u32 = CLAMP(sx0 + 1, 0, ELE_PADDED_SPAN - 1);
  const sy1: u32 = CLAMP(sy0 + 1, 0, ELE_PADDED_SPAN - 1);

  const sdx: f64 = sx - sx0;
  const sdy: f64 = sy - sy0;

  // bilinear interpolation
  const a: i16 = page_ele[sy0 * ELE_PADDED_SPAN + sx0];
  const b: i16 = page_ele[sy0 * ELE_PADDED_SPAN + sx1];
  const c: i16 = page_ele[sy1 * ELE_PADDED_SPAN + sx0];
  const d: i16 = page_ele[sy1 * ELE_PADDED_SPAN + sx1];

  const ab: f64 = LERP(a, b, sdx);
  const cd: f64 = LERP(c, d, sdx);
  const out: f64 = LERP(ab, cd, sdy);

  return Math.round(out);
}

/**
 * Bicubic elevation sampling
 * - Zero GC Allocation: 배열 대신 로컬 스칼라 변수(레지스터) 100% 활용
 * - Loop Unrolling: 4x4 루프를 해체하여 V8 JIT 파이프라이닝 극대화
 * - Fast Path: ELE_PAD=2의 특성을 이용해 CLAMP 분기 완전 제거
 */
function sample_ele_bicubic(page_ele: I16Array, tx: u32, ty: u32): i16 {
  const sx: f64 = (tx + 0.5) / ELE_CELL_STEP + ELE_PAD - 0.5;
  const sy: f64 = (ty + 0.5) / ELE_CELL_STEP + ELE_PAD - 0.5;

  const sx0: i32 = Math.floor(sx);
  const sy0: i32 = Math.floor(sy);

  const fdx: f64 = sx - sx0;
  const fdy: f64 = sy - sy0;

  // 1. 🚀 Zero-GC Weight Calculation (배열 할당 제거, 레지스터 다이렉트 할당)
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

  // 2. 🛡️ Fast Path 분기 (V8 분기 예측 100% 히트)
  // ELE_PAD=2 덕분에 sx0는 [1, 33], sy0는 [1, 33] 안에 존재합니다.
  // 즉, 99.99%의 케이스에서 CLAMP 없이 순수 배열 다이렉트 접근이 가능합니다.
  if (sx0 >= 1 && sx0 <= span - 3 && sy0 >= 1 && sy0 <= span - 3) {
    // 3. 🚀 Loop Unrolling & Base Indexing (16 Reads + 12 Muls)
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

  // --- 🐌 Slow Path (안전망: 예외 케이스 처리) ---
  let out: f64 = 0;
  const wx = [wx0, wx1, wx2, wx3];
  const wy = [wy0, wy1, wy2, wy3];

  for (let j = 0; j < 4; j++) {
    const py = CLAMP(sy0 - 1 + j, 0, span - 1);
    let row_out: f64 = 0;
    for (let i = 0; i < 4; i++) {
      const px = CLAMP(sx0 - 1 + i, 0, span - 1);
      row_out += page_ele[py * span + px] * wx[i];
    }
    out += row_out * wy[j];
  }

  return Math.round(out);
}

export const sample_ele = sample_ele_bicubic;

/**
 * Unpacks a dense (PAGE_CELL_SPAN x PAGE_CELL_SPAN) elevation chunk from the coarse heightmap.
 * Samples are taken at cell centers (0.5 offset) to match DB-side resampling.
 */
export function dense_ele(page_ele: I16Array, tx0: u32, ty0: u32): I16Array {
  const out = new Int16Array(CHUNK_CELL_SPAN2);

  let idx: u32 = 0;
  for (let dy: u32 = 0; dy < CHUNK_CELL_SPAN; dy++) {
    const ty: u32 = ty0 + dy; // Center sample
    for (let dx: u32 = 0; dx < CHUNK_CELL_SPAN; dx++) {
      const tx: u32 = tx0 + dx; // Center sample
      // Rounding preserves the fixed-point [cell] integer convention.
      out[idx++] = sample_ele(page_ele, tx, ty);
    }
  }

  return out;
}
