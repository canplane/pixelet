// @pxl/web/reference/volume.ts

/**
 * Canonical render-time Volume for one 64x64 chunk.
 *
 * This is intentionally not the sparse per-cell SVO/octree source format objects
 * are stored in. World layers merge into whole 64x64x64 signed-Z bands, then
 * meshing and raycasting read the same flat typed arrays directly. A band is
 * allocated in full even if only one peak or object touches it; sparsity
 * exists only between Z bands.
 *
 * The volume lives in the module (crates/pxl-core/src/bake/volume.rs): its bands
 * are blocks of the core memory the module took from the pools, and every
 * paint here is a call into it. What a reader gets back — `bands` — is
 * views over those blocks and over the module's own overflow lanes, taken
 * when read and again after any paint, so the TypeScript scan and the
 * raycast read the bands where they are, and the Rust scan reads the
 * volume by its id.
 */

import type { i32, u32 } from "@xpute/core/kit/word.ts";

import { CHUNK_CELL_SPAN } from "@pxl/shared/bake/constant.ts";

import { CORE_PXL } from "@/wasm/core_pool.ts";
import { pack_material, pack_sub_cell, unpack_material, type VoxelMaterial } from "@/render/mesh/voxel.ts";

export const L2_BAND_SHAMT: u32 = 6;
export const L2_BAND_CELL_SPAN: u32 = CHUNK_CELL_SPAN;
export const L2_BAND_CELL_MASK: u32 = L2_BAND_CELL_SPAN - 1;
export const L2_BAND_CELL_SPAN2: u32 = L2_BAND_CELL_SPAN * L2_BAND_CELL_SPAN;
export const L2_BAND_VOXEL_CNT: u32 = L2_BAND_CELL_SPAN2 * L2_BAND_CELL_SPAN;
if (L2_BAND_CELL_SPAN !== 1 << L2_BAND_SHAMT) {
  throw new Error("render: render band span must stay power-of-two aligned");
}

/** Column-major band-local idx: Z is contiguous for paint/mesh/raycast. */
export function band_voxel_idx(x: u32, y: u32, z: u32): u32 {
  return (y * L2_BAND_CELL_SPAN + x) * L2_BAND_CELL_SPAN + z;
}

/**
 * The rare overflow when several non-overlapping sub-cell objects share one
 * base voxel: the first takes the voxel, the rest are listed here. `col` is
 * the band-local column (`cy * span + cx`), `z` the absolute cell-space Z,
 * `material` packed as the voxels are, `sub` the four sub-cell bytes (ox,
 * oy, oz, sz) in one word. Sorted by column once the volume is sealed, so a
 * column's own entries are one binary search away. Views over the module's
 * lanes, valid until the next paint.
 */
export type SubVoxels = {
  readonly len: u32;
  readonly col: Uint32Array;
  readonly z: Int32Array;
  readonly material: Uint32Array;
  readonly layer_idx: Uint8Array;
  readonly sub: Uint32Array;
};

export type VoxelBand = {
  /** Signed Z-band coordinate. */
  band_z: i32;
  /** 0xCC_RRGGBB. Zero is AIR; one direct read yields class and RGB. */
  material: Uint32Array;
  /** Texture-array layer, kept separate so the direct payload is 5 B/voxel. */
  layer_idx: Uint8Array;
  /** Present only for bands containing sub-cell objects. */
  sub_cell: Uint8Array | null;
  /** Rare overflow when multiple non-overlapping SVOs share one base voxel. */
  extra: SubVoxels;
};

export type ChunkVolume = {
  /** The module's id for it. */
  readonly id: u32;
  /** Small ascending-Z array; normally one or two dense bands. Views taken
   * from the module when read, and again after any paint. */
  readonly bands: readonly VoxelBand[];
  sealed: boolean;
  /** A paint was refused for want of a block — no pool had room for a band
   * — so the volume is not the composite that was asked for. */
  starved: boolean;
};

class ModuleVolume implements ChunkVolume {
  sealed = false;
  starved = false;
  private views: VoxelBand[] | null = null;

  constructor(readonly id: u32) {}

  /** A paint may have added a band or grown an overflow lane, moving it. */
  painted(ok: u32): void {
    if (this.sealed) throw new Error("render: cannot mutate a sealed chunk volume");
    if (ok === 0) this.starved = true;
    this.views = null;
  }

  get bands(): readonly VoxelBand[] {
    return this.views ??= band_views(this.id);
  }

  released(): void {
    this.views = [];
    this.sealed = true;
  }
}

function band_views(id: u32): VoxelBand[] {
  const p = pxl();
  const count = p.pxl_volume_bands(id);
  const buf = p.memory.buffer;
  const bands: VoxelBand[] = [];
  for (let i = 0; i < count; i++) {
    const row = new Uint32Array(buf, p.pxl_volume_band(id, i), BAND_WORDS);
    const n = row[4];
    bands.push({
      band_z: row[0] | 0,
      material: new Uint32Array(buf, row[1], L2_BAND_VOXEL_CNT),
      layer_idx: new Uint8Array(buf, row[2], L2_BAND_VOXEL_CNT),
      sub_cell: row[3] === 0 ? null : new Uint8Array(buf, row[3], L2_BAND_VOXEL_CNT * 4),
      extra: {
        len: n,
        col: new Uint32Array(buf, row[5], n),
        z: new Int32Array(buf, row[6], n),
        material: new Uint32Array(buf, row[7], n),
        layer_idx: new Uint8Array(buf, row[8], n),
        sub: new Uint32Array(buf, row[9], n),
      },
    });
  }
  return bands;
}

export type OrderedVoxelBand = VoxelBand;

/** Words a band's row holds: crates/pxl-core/src/bake/volume.rs's BAND_WORDS. */
const BAND_WORDS = 10;

function pxl() {
  if (CORE_PXL === null) throw new Error("render: a volume needs the core memory and the module over it");
  return CORE_PXL;
}

export function alloc_chunk_volume(): ChunkVolume {
  return new ModuleVolume(pxl().pxl_volume_open());
}

/** Returns a consumed composition to the module: its bands to the stores
 * up to their cap, and everything else to the pools. Every view is dead
 * afterwards. */
export function release_chunk_volume(volume: ChunkVolume): void {
  pxl().pxl_volume_release(volume.id);
  (volume as ModuleVolume).released();
}

/** The band at `band_z`, or undefined. A chunk is one column of bands, so
 * the signed Z index is the whole key. */
export function get_chunk_band(volume: ChunkVolume, band_z: i32): VoxelBand | undefined {
  for (const band of volume.bands) if (band.band_z === band_z) return band;
  return undefined;
}

export function chunk_band_zs(volume: ChunkVolume): i32[] {
  return volume.bands.map((band) => band.band_z);
}

/** Compatibility scan view; the backing array is already ascending-Z. */
export function ordered_chunk_bands(volume: ChunkVolume): readonly OrderedVoxelBand[] {
  return volume.bands;
}

/** Closes the volume to painting and orders every band's extra sub-voxels
 * by column, which is what lets a column find its own during the scan. */
export function seal_chunk_volume(volume: ChunkVolume): ChunkVolume {
  pxl().pxl_volume_seal(volume.id);
  (volume as ModuleVolume).painted(1);
  volume.sealed = true;
  return volume;
}

/** The range [lo, hi) of a band's extra sub-voxels in column `col`, once
 * the volume is sealed; [0, 0) when it has none there. */
export function sub_voxel_range(extra: SubVoxels, col: u32): [u32, u32] {
  const cols = extra.col;
  let lo = 0, hi = extra.len;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (cols[mid] < col) lo = mid + 1;
    else hi = mid;
  }
  let end = lo;
  while (end < extra.len && cols[end] === col) end++;
  return [lo, end];
}

function painted(volume: ChunkVolume, ok: u32): void {
  (volume as ModuleVolume).painted(ok);
}

/** What a paint made in the module directly (a terrain chunk, say) tells
 * the volume: whether it was refused, and that its views are stale. */
export function note_paint(volume: ChunkVolume, ok: u32): void {
  painted(volume, ok);
}

/** Full replacement at one voxel: later WorldLayers overwrite earlier ones. */
export function set_chunk_voxel(
  volume: ChunkVolume,
  cx: u32,
  cy: u32,
  z: i32,
  material: VoxelMaterial,
  sub_cell?: readonly [ox: u32, oy: u32, oz: u32, sz: u32],
): void {
  const sub = sub_cell ? pack_sub_cell(sub_cell[0], sub_cell[1], sub_cell[2], sub_cell[3]) : 0;
  painted(volume, pxl().pxl_volume_set_voxel(volume.id, cx, cy, z, pack_material(material), material.layer_idx, sub));
}

/** Additional SVO sharing a base voxel already claimed within the same layer. */
export function append_chunk_sub_voxel(volume: ChunkVolume, cx: u32, cy: u32, z: i32, material: VoxelMaterial, sub: u32): void {
  painted(volume, pxl().pxl_volume_append_sub_voxel(volume.id, cx, cy, z, pack_material(material), material.layer_idx, sub));
}

/** Paints one solid column across as many whole signed-Z bands as needed. */
export function fill_chunk_column(
  volume: ChunkVolume,
  cx: u32,
  cy: u32,
  z0: i32,
  z1: i32,
  material: VoxelMaterial,
): void {
  if (z1 < z0) return;
  painted(volume, pxl().pxl_volume_fill_column(volume.id, cx, cy, z0, z1, pack_material(material), material.layer_idx));
}

/** Direct material lookup by band-local voxel idx. */
export function band_material(band: VoxelBand, voxel_idx: u32): VoxelMaterial {
  return unpack_material(band.material[voxel_idx], band.layer_idx[voxel_idx]);
}
