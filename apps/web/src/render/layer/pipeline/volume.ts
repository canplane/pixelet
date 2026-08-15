// @pxl/web/render/layer/pipeline/volume.ts

/**
 * Canonical render-time Volume for one 64x64 chunk.
 *
 * This is intentionally not the sparse per-cell SVO/octree source format objects
 * are stored in. World layers merge into whole 64x64x64 signed-Z bands, then
 * meshing and raycasting read the same flat typed arrays directly. A band is
 * allocated in full even if only one peak or object touches it; sparsity
 * exists only between Z bands.
 */

import type { f64, i32, u32 } from "@xpute/core/kit/word.ts";

import { CHUNK_CELL_SPAN } from "@pxl/shared/bake/constants.ts";

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

export type SubVoxelEntry = VoxelMaterial & {
  cx: u32;
  cy: u32;
  /** Absolute cell-space Z, not relative to a transient terrain floor. */
  z: i32;
  sub_sz: u32;
  sub_ox: u32;
  sub_oy: u32;
  sub_oz: u32;
};

export type VoxelBand = {
  /** Signed Z-band coordinate. */
  band_z: i32;
  /** 0xCC_RRGGBB. Zero is AIR; one direct read yields class and RGB. */
  material: Uint32Array;
  /** Texture-array layer, kept separate so the direct payload is 5 B/voxel. */
  layer_idx: Uint8Array;
  /** Lazily allocated only for bands containing sub-cell objects. */
  sub_cell: Uint8Array | null;
  /** Rare overflow when multiple non-overlapping SVOs share one base voxel. */
  extra_sub_voxels: SubVoxelEntry[];
};

export type ChunkVolume = {
  /** Small ascending-Z array; normally one or two dense bands. */
  bands: VoxelBand[];
  min_band_z: i32;
  max_band_z: i32;
  sealed: boolean;
};

export type OrderedVoxelBand = VoxelBand;

/** Two scratch bands cap retained CPU scratch storage at about 2.5 MiB. */
const MAX_POOLED_BANDS: u32 = 2;
const band_pool: VoxelBand[] = [];

export function alloc_chunk_volume(): ChunkVolume {
  return { bands: [], min_band_z: Infinity, max_band_z: -Infinity, sealed: false };
}

/** Returns a consumed temporary composition to the tiny renderer scratch pool. */
export function release_chunk_volume(volume: ChunkVolume): void {
  for (const band of volume.bands) {
    if (band_pool.length >= MAX_POOLED_BANDS) break;
    band.extra_sub_voxels.length = 0;
    // A rare 1 MiB SVO payload must not become permanent scratch overhead.
    band.sub_cell = null;
    band_pool.push(band);
  }
  volume.bands = [];
  volume.sealed = true;
}

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

export function get_chunk_band(
  volume: ChunkVolume,
  band_x: u32,
  band_y: u32,
  band_z: i32,
): VoxelBand | undefined {
  if (band_x !== 0 || band_y !== 0) return undefined;
  return volume.bands.find((band) => band.band_z === band_z);
}

export function chunk_band_zs(volume: ChunkVolume): i32[] {
  return volume.bands.map((band) => band.band_z);
}

/** Compatibility scan view; the backing array is already ascending-Z. */
export function ordered_chunk_bands(volume: ChunkVolume): readonly OrderedVoxelBand[] {
  return volume.bands;
}

/** Marks a cached composition immutable. No palette/finalization work exists. */
export function seal_chunk_volume(volume: ChunkVolume): ChunkVolume {
  volume.sealed = true;
  return volume;
}

function get_or_create_band(volume: ChunkVolume, band_z: i32): VoxelBand {
  if (volume.sealed) throw new Error("render: cannot mutate a sealed chunk volume");
  let insert_at = 0;
  while (insert_at < volume.bands.length && volume.bands[insert_at].band_z < band_z) insert_at++;
  const existing = volume.bands[insert_at];
  if (existing?.band_z === band_z) return existing;

  const band = band_pool.pop() ?? {
    band_z,
    material: new Uint32Array(L2_BAND_VOXEL_CNT),
    layer_idx: new Uint8Array(L2_BAND_VOXEL_CNT),
    sub_cell: null,
    extra_sub_voxels: [],
  };
  band.band_z = band_z;
  band.material.fill(0);
  band.layer_idx.fill(NO_LAYER);
  band.sub_cell = null;
  band.extra_sub_voxels.length = 0;
  volume.bands.splice(insert_at, 0, band);
  volume.min_band_z = Math.min(volume.min_band_z, band_z);
  volume.max_band_z = Math.max(volume.max_band_z, band_z);
  return band;
}

function locate_voxel(cx: u32, cy: u32, z: i32): {
  band_z: i32;
  local_z: u32;
  idx: u32;
} {
  const band_z = Math.floor(z / L2_BAND_CELL_SPAN);
  const local_z = z - band_z * L2_BAND_CELL_SPAN;
  return { band_z, local_z, idx: band_voxel_idx(cx, cy, local_z) };
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
  const loc = locate_voxel(cx, cy, z);
  const band = get_or_create_band(volume, loc.band_z);
  band.material[loc.idx] = pack_material(material);
  band.layer_idx[loc.idx] = material.layer_idx;

  if (band.extra_sub_voxels.length > 0) {
    band.extra_sub_voxels = band.extra_sub_voxels.filter((entry) => entry.cx !== cx || entry.cy !== cy || entry.z !== z);
  }

  if (sub_cell) {
    band.sub_cell ??= new Uint8Array(L2_BAND_VOXEL_CNT * 4);
    const off = loc.idx * 4;
    band.sub_cell[off] = sub_cell[0];
    band.sub_cell[off + 1] = sub_cell[1];
    band.sub_cell[off + 2] = sub_cell[2];
    band.sub_cell[off + 3] = sub_cell[3];
  } else if (band.sub_cell) {
    band.sub_cell.fill(0, loc.idx * 4, loc.idx * 4 + 4);
  }
}

/** Additional SVO sharing a base voxel already claimed within the same layer. */
export function append_chunk_sub_voxel(volume: ChunkVolume, entry: SubVoxelEntry): void {
  const loc = locate_voxel(entry.cx, entry.cy, entry.z);
  const band = get_or_create_band(volume, loc.band_z);
  band.extra_sub_voxels.push(entry);
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
  const first_band_z = Math.floor(z0 / L2_BAND_CELL_SPAN);
  const last_band_z = Math.floor(z1 / L2_BAND_CELL_SPAN);
  const packed = pack_material(material);

  for (let band_z = first_band_z; band_z <= last_band_z; band_z++) {
    const band = get_or_create_band(volume, band_z);
    const local_z0 = band_z === first_band_z ? z0 - band_z * L2_BAND_CELL_SPAN : 0;
    const local_z1 = band_z === last_band_z ? z1 - band_z * L2_BAND_CELL_SPAN : L2_BAND_CELL_SPAN - 1;
    const start = band_voxel_idx(cx, cy, local_z0);
    const end = band_voxel_idx(cx, cy, local_z1) + 1;
    band.material.fill(packed, start, end);
    band.layer_idx.fill(material.layer_idx, start, end);
    if (band.sub_cell) band.sub_cell.fill(0, start * 4, end * 4);
  }
}

/** Direct material lookup by band-local voxel idx. */
export function band_material(band: VoxelBand, voxel_idx: u32): VoxelMaterial {
  return unpack_material(band.material[voxel_idx], band.layer_idx[voxel_idx]);
}

export const SUB_CELL_NONE: u32 = 0;
export const SUB_CELL_SCALE: u32 = 256;

export function pack_sub_cell_fraction(fraction: f64): u32 {
  return Math.min(SUB_CELL_SCALE - 1, Math.max(0, Math.round(fraction * SUB_CELL_SCALE)));
}

export function unpack_sub_cell_fraction(packed: u32): f64 {
  return packed / SUB_CELL_SCALE;
}
