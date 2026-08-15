// @pxl/web/render/layer/pipeline/chunk_surface.ts

/** Compact LOD-independent top-surface Tile for one 64x64 chunk. */

import type { i32, u32 } from "@xpute/core/kit/word.ts";

import { NO_LAYER } from "./volume.ts";
import { CHUNK_CELL_SPAN } from "@pxl/shared/bake/constants.ts";

const CHUNK_CELL_SPAN2: u32 = CHUNK_CELL_SPAN * CHUNK_CELL_SPAN;

/** 36 KiB: 4096 * (4-byte Z + 4-byte packed material + 1-byte layer). */
export type ChunkSurface = {
  z: Int32Array;
  material: Uint32Array;
  layer_idx: Uint8Array;
};

export function alloc_chunk_surface(): ChunkSurface {
  const layer_idx = new Uint8Array(CHUNK_CELL_SPAN2);
  layer_idx.fill(NO_LAYER);
  return {
    z: new Int32Array(CHUNK_CELL_SPAN2),
    material: new Uint32Array(CHUNK_CELL_SPAN2),
    layer_idx,
  };
}

/** Higher pixels win; later WorldLayers win when they occupy the same Z. */
export function set_surface_pixel(
  surface: ChunkSurface,
  cx: u32,
  cy: u32,
  z: i32,
  packed_material: u32,
  layer_idx: u32,
): void {
  const idx = cy * CHUNK_CELL_SPAN + cx;
  if (surface.material[idx] !== 0 && z < surface.z[idx]) return;
  surface.z[idx] = z;
  surface.material[idx] = packed_material;
  surface.layer_idx[idx] = layer_idx;
}
