// @pxl/web/render/mesh/voxel.ts

/**
 * What one voxel's bytes mean: its material class and colour packed into a
 * word, its texture layer, and the inset of a box that fills only part of
 * its cell. Pure encoding, shared by the volume that stores voxels, the
 * scan that reads them into runs and the mesher that draws the runs — and
 * free of any allocation, so a thread with a memory of its own can import
 * it without importing the core's.
 */

import type { f64, u32 } from "@xpute/core/kit/word.ts";

/** No texture-array layer applies; use the material's flat RGB. */
export const NO_LAYER: u32 = 255;

export const enum MaterialClass {
  AIR = 0,
  LAND = 1,
  WATER = 2,
  TRANSPORT = 3,
  STRUCTURE = 4,
  OBJECT = 5,
}

export type VoxelMaterial = {
  material_class: MaterialClass;
  r: u32;
  g: u32;
  b: u32;
  layer_idx: u32;
};

export function pack_material(material: VoxelMaterial): u32 {
  return (
    ((material.material_class & 255) << 24) |
    ((material.r & 255) << 16) |
    ((material.g & 255) << 8) |
    (material.b & 255)
  ) >>> 0;
}

export function unpack_material(packed: u32, layer_idx: u32): VoxelMaterial {
  return {
    material_class: (packed >>> 24) as MaterialClass,
    r: (packed >>> 16) & 255,
    g: (packed >>> 8) & 255,
    b: packed & 255,
    layer_idx,
  };
}

export const pack_sub_cell = (ox: u32, oy: u32, oz: u32, sz: u32): u32 => (ox | (oy << 8) | (oz << 16) | (sz << 24)) >>> 0;

export const SUB_CELL_NONE: u32 = 0;
export const SUB_CELL_SCALE: u32 = 256;

export function pack_sub_cell_fraction(fraction: f64): u32 {
  return Math.min(SUB_CELL_SCALE - 1, Math.max(0, Math.round(fraction * SUB_CELL_SCALE)));
}

export function unpack_sub_cell_fraction(packed: u32): f64 {
  return packed / SUB_CELL_SCALE;
}

/**
 * The largest packed value, read as exactly 1 rather than 255/256.
 *
 * An offset of 1 is meaningless, so `unpack_sub_cell_fraction` never needs
 * to express it. A *size* of 1 is not: coarse LOD fits a box to what is
 * actually inside a cell, and a box that fills one axis while shrinking
 * another is the ordinary case, not an edge one. Reading the top value as
 * full costs a comparison and keeps 0 free as SUB_CELL_NONE.
 */
export const SUB_CELL_FULL: u32 = SUB_CELL_SCALE - 1;

export function unpack_sub_cell_size(packed: u32): f64 {
  return packed === SUB_CELL_FULL ? 1 : packed / SUB_CELL_SCALE;
}
