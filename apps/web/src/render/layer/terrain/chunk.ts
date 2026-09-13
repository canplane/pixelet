// @pxl/web/render/layer/terrain/chunk.ts

/**
 * Coordinate prefix rules:
 * - c (Chunk)  : chunk lattice index (e.g., cx, cy)
 * - d (Delta)  : local loop delta (e.g., dx, dy)
 * - t (Texel)  : continuous tile texel / pixel coordinates (e.g., tx, ty)
 * - s (Source) : source-grid coordinates in the padded low-res array (e.g., sx, sy)
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import type { I16Array, U8Array } from "@xpute/core/kit/array.ts";

import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L2_SHAMT, L3_SHAMT } from "@pxl/shared/spec/space/indexer.ts";

import { type Ground, type Terrain } from "@pxl/shared/bake/format.ts";
import { CELL_SHAMT, CHUNK_CELL_SPAN, PAGE_CELL_SPAN } from "@pxl/shared/bake/constant.ts";
import { FIXED_ALLOC } from "@/render/layer/pipeline/core_pool.ts";
import { dense_ele_region } from "./elevation.ts";

const PAGE_CHUNK_SPAN: u32 = 1 << (L3_SHAMT - L2_SHAMT);

function slice_ground_ch(ch: U8Array, tx0: u32, ty0: u32, span: u32, out: U8Array): U8Array {
  let dst_off: u32 = 0;
  for (let dy: u32 = 0; dy < span; dy++) {
    const src_off: u32 = (ty0 + dy) * PAGE_CELL_SPAN + tx0;
    out.set(ch.subarray(src_off, src_off + span), dst_off);
    dst_off += span;
  }

  return out;
}

function page_off_of(cx: u32, cy: u32): IVec {
  const tx0: u32 = cx * CHUNK_CELL_SPAN; // texel space
  const ty0: u32 = cy * CHUNK_CELL_SPAN;
  const page_off_x: u32 = tx0 >> -CELL_SHAMT; // in u-space
  const page_off_y: u32 = ty0 >> -CELL_SHAMT;
  return ivec.gather(BigInt(page_off_x), BigInt(page_off_y), BigInt(0n));
}

/**
 * Every chunk offset in a page, cheap position-only enumeration (no ground/
 * elevation slicing) — the same PAGE_CHUNK_SPAN^2 list every time (a pure
 * function of compile-time constants), computed once at module load and
 * handed back by reference on every call. Callers filter/order this by
 * whatever's relevant (e.g. real-world distance from camera,
 * chunk_stage.tsx) *before* paying slice_chunk's real per-chunk cost,
 * rather than slicing every chunk in a page up front only to throw most of
 * them away.
 */
const CHUNK_OFFSETS: readonly IVec[] = (() => {
  const out: IVec[] = [];
  for (let cy: u32 = 0; cy < PAGE_CHUNK_SPAN; cy++) {
    for (let cx: u32 = 0; cx < PAGE_CHUNK_SPAN; cx++) {
      out.push(page_off_of(cx, cy));
    }
  }
  return out;
})();

export function chunk_offsets(): readonly IVec[] {
  return CHUNK_OFFSETS;
}

/**
 * One chunk of a page's terrain, as the adapter reads it: the four ground
 * channels and the dense elevation, chunk-local.
 *
 * Written into one set of scratch lanes and handed back as views over them,
 * so the result is valid until the next call and must be consumed before
 * it — which both callers do, in the same synchronous pass. It used to be
 * five fresh arrays, 24 KB of heap per call, and the call is made once per
 * composite and once per direct build: a zoom-out that rebuilds two
 * thousand chunks moved fifty megabytes through the collector for data
 * that is read once and dropped.
 *
 * The elevation half is the real cost — a bicubic sample per cell
 * (elevation.ts) — and is what makes this worth calling only for chunks
 * already decided on.
 */
const SLICE_CELLS = CHUNK_CELL_SPAN * CHUNK_CELL_SPAN;
const SLICE_GROUND: Ground = {
  land: FIXED_ALLOC.take(Uint8Array, SLICE_CELLS),
  water: FIXED_ALLOC.take(Uint8Array, SLICE_CELLS),
  structure: FIXED_ALLOC.take(Uint8Array, SLICE_CELLS),
  transport: FIXED_ALLOC.take(Uint8Array, SLICE_CELLS),
};
const SLICE_ELE = FIXED_ALLOC.take(Int16Array, SLICE_CELLS);
const SLICE: Terrain = { ground: SLICE_GROUND, ele: SLICE_ELE };

export function slice_chunk(page_ground: Ground, page_ele: I16Array, page_off: IVec): Terrain {
  const tx0: u32 = Number(ivec.X(page_off)) << -CELL_SHAMT;
  const ty0: u32 = Number(ivec.Y(page_off)) << -CELL_SHAMT;
  const span = CHUNK_CELL_SPAN;
  slice_ground_ch(page_ground.land, tx0, ty0, span, SLICE_GROUND.land);
  slice_ground_ch(page_ground.water, tx0, ty0, span, SLICE_GROUND.water);
  slice_ground_ch(page_ground.structure, tx0, ty0, span, SLICE_GROUND.structure);
  slice_ground_ch(page_ground.transport, tx0, ty0, span, SLICE_GROUND.transport);
  // ground first: water-family cells get their elevation recess applied
  // here (elevation.ts's WATER_RECESS_CELLS), read off the slice just made.
  dense_ele_region(page_ele, tx0, ty0, span, SLICE_GROUND, SLICE_ELE);
  return SLICE;
}
