// @pixelet/client/render/terrain/chunk.ts

/**
 * Coordinate prefix rules:
 * - c (Chunk)  : chunk lattice index (e.g., cx, cy)
 * - d (Delta)  : local loop delta (e.g., dx, dy)
 * - t (Texel)  : continuous tile texel / pixel coordinates (e.g., tx, ty)
 * - s (Source) : source-grid coordinates in the padded low-res array (e.g., sx, sy)
 */

import type { u32 } from "@pixelet/shared/kit/word.ts";
import type { I16Array, U8Array } from "@pixelet/shared/kit/array.ts";

import { type IVec, ivec } from "@pixelet/shared/spec/space/pvf3/mod.ts";
import { L2_SHAMT, L3_SHAMT } from "@pixelet/shared/spec/space/indexer.ts";

import { type Ground, type Terrain } from "@pixelet/shared/bake/format.ts";
import { CELL_SHAMT, CHUNK_CELL_SPAN, PAGE_CELL_SPAN } from "@pixelet/shared/bake/constants.ts";
import { dense_ele } from "./elevation.ts";

const PAGE_CHUNK_SPAN: u32 = 1 << (L3_SHAMT - L2_SHAMT);

const CHUNK_CELL_SPAN2: u32 = CHUNK_CELL_SPAN * CHUNK_CELL_SPAN;

function slice_ground_ch(ch: U8Array, tx0: u32, ty0: u32): U8Array {
  const out = new Uint8Array(CHUNK_CELL_SPAN2);

  let dst_off: u32 = 0;
  for (let dy: u32 = 0; dy < CHUNK_CELL_SPAN; dy++) {
    const src_off: u32 = (ty0 + dy) * PAGE_CELL_SPAN + tx0;
    out.set(ch.subarray(src_off, src_off + CHUNK_CELL_SPAN), dst_off);
    dst_off += CHUNK_CELL_SPAN;
  }

  return out;
}

export function page_to_chunks(page_ground: Ground, page_ele: I16Array): [IVec, Terrain][] {
  const entries: [IVec, Terrain][] = [];

  for (let cy: u32 = 0; cy < PAGE_CHUNK_SPAN; cy++) {
    for (let cx: u32 = 0; cx < PAGE_CHUNK_SPAN; cx++) {
      const tx0: u32 = cx * CHUNK_CELL_SPAN; // texel space
      const ty0: u32 = cy * CHUNK_CELL_SPAN;

      const ground: Ground = {
        land: slice_ground_ch(page_ground.land, tx0, ty0),
        water: slice_ground_ch(page_ground.water, tx0, ty0),
        structure: slice_ground_ch(page_ground.structure, tx0, ty0),
        transport: slice_ground_ch(page_ground.transport, tx0, ty0),
      };
      const ele: I16Array = dense_ele(page_ele, tx0, ty0);

      const page_off_x: u32 = tx0 >> -CELL_SHAMT; // in u-space
      const page_off_y: u32 = ty0 >> -CELL_SHAMT;
      const page_off: IVec = ivec.gather(BigInt(page_off_x), BigInt(page_off_y), BigInt(0n));
      entries.push([page_off, { ground, ele }]);
    }
  }

  return entries;
}
