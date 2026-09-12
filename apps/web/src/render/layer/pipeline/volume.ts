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
import type { Block } from "@xpute/core/ds/buddy.ts";
import { Vector } from "@xpute/core/ds/arena.ts";
import type { I32Array, U32Array, U8Array } from "@xpute/core/kit/array.ts";

import { CHUNK_CELL_SPAN } from "@pxl/shared/bake/constant.ts";

import { CORE_ALLOC, CORE_POOL } from "./core_pool.ts";

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

/**
 * The rare overflow when several non-overlapping sub-cell objects share one
 * base voxel: the first takes the voxel, the rest are listed here, as lanes
 * on the band. `col` is the band-local column (`cy * span + cx`), `z` the
 * absolute cell-space Z, `material` packed as the voxels are, `sub` the
 * four sub-cell bytes (ox, oy, oz, sz) in one word. Sorted by column once
 * the volume is sealed, so a column's own entries are one binary search
 * away — what a per-(band, column) map of arrays used to be built for on
 * every scan.
 */
export type SubVoxels = {
  col: Vector<U32Array>;
  z: Vector<I32Array>;
  material: Vector<U32Array>;
  layer_idx: Vector<U8Array>;
  sub: Vector<U32Array>;
};

const EXTRA_LANE = { init_cap: 8, alloc: CORE_ALLOC } as const;

function new_sub_voxels(): SubVoxels {
  return {
    col: new Vector(Uint32Array, EXTRA_LANE),
    z: new Vector(Int32Array, EXTRA_LANE),
    material: new Vector(Uint32Array, EXTRA_LANE),
    layer_idx: new Vector(Uint8Array, EXTRA_LANE),
    sub: new Vector(Uint32Array, EXTRA_LANE),
  };
}

function clear_sub_voxels(extra: SubVoxels): void {
  extra.col.truncate(0);
  extra.z.truncate(0);
  extra.material.truncate(0);
  extra.layer_idx.truncate(0);
  extra.sub.truncate(0);
}

export const pack_sub_cell = (ox: u32, oy: u32, oz: u32, sz: u32): u32 => (ox | (oy << 8) | (oz << 16) | (sz << 24)) >>> 0;

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
  extra: SubVoxels;
  /** The core pool blocks behind the three arrays above — see
   * release_chunk_volume for when each goes back. */
  blocks: { material: Block; layer_idx: Block; sub_cell: Block | null };
};

export type ChunkVolume = {
  /** Small ascending-Z array; normally one or two dense bands. */
  bands: VoxelBand[];
  min_band_z: i32;
  max_band_z: i32;
  sealed: boolean;
};

export type OrderedVoxelBand = VoxelBand;

/**
 * How many band buffers to keep for reuse.
 *
 * A band is `L2_BAND_VOXEL_CNT` of material plus the same again of layer
 * index — 1.31MB — and a composite produces several. At 2 the pool covered
 * the first two and every one after that was a fresh pair of typed arrays,
 * which was fine when compositing was rare and is not now: measured, ~38
 * chunk scans a second against a heap growing 188MB/s with the chunk layer
 * itself retaining almost nothing. It was transient, and this was it.
 *
 * Only one composite is live at a time (with_temporary_composite is
 * synchronous) plus whatever the raycast cache holds, so the pool needs
 * roughly one chunk's worth of bands. `scan_bands_max` in the ledger is the
 * number to size this against; 16 is ~21MB of standing scratch, which is
 * nothing next to what it stops churning.
 *
 * The arrays are core pool blocks (core_pool.ts) rather than typed arrays of
 * their own: `material` is exactly one 1 MiB block and `layer_idx` one
 * 256 KiB block. A band past this cap, and every band's `sub_cell`, hands
 * its blocks back to the pool instead of becoming garbage — which is what
 * the 1 MiB `sub_cell` did on every reuse before.
 */
const MAX_POOLED_BANDS: u32 = 16;

function borrow<T>(make: (buf: ArrayBuffer, offset: number, length: number) => T, length: u32, bytes_per: u32): { block: Block; view: T } {
  const block = CORE_POOL.acquire(length * bytes_per);
  return { block, view: make(block.buf, block.byte_offset, length) };
}

function new_band(band_z: i32): VoxelBand {
  const material = borrow((b, o, n) => new Uint32Array(b, o, n), L2_BAND_VOXEL_CNT, 4);
  const layer_idx = borrow((b, o, n) => new Uint8Array(b, o, n), L2_BAND_VOXEL_CNT, 1);
  return {
    band_z,
    material: material.view,
    layer_idx: layer_idx.view,
    sub_cell: null,
    extra: new_sub_voxels(),
    blocks: { material: material.block, layer_idx: layer_idx.block, sub_cell: null },
  };
}

function drop_sub_cell(band: VoxelBand): void {
  if (band.blocks.sub_cell) CORE_POOL.release(band.blocks.sub_cell);
  band.blocks.sub_cell = null;
  band.sub_cell = null;
}

/** Dev-only: how often a band came from the pool rather than being built.
 * A low reuse share means MAX_POOLED_BANDS is under a composite's real band
 * count and the difference is pure garbage. */
export const band_pool_stats = { allocated: 0, reused: 0 };
const band_pool: VoxelBand[] = [];

export function alloc_chunk_volume(): ChunkVolume {
  return { bands: [], min_band_z: Infinity, max_band_z: -Infinity, sealed: false };
}

/** Returns a consumed temporary composition: bands to the scratch pool up
 * to its cap, and everything else to the core pool. */
export function release_chunk_volume(volume: ChunkVolume): void {
  for (const band of volume.bands) {
    clear_sub_voxels(band.extra);
    // A rare 1 MiB SVO payload must not become permanent scratch overhead.
    drop_sub_cell(band);
    if (band_pool.length < MAX_POOLED_BANDS) {
      band_pool.push(band);
      continue;
    }
    CORE_POOL.release(band.blocks.material);
    CORE_POOL.release(band.blocks.layer_idx);
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
/** Closes the volume to painting and orders every band's extra sub-voxels
 * by column, which is what lets a column find its own during the scan. */
export function seal_chunk_volume(volume: ChunkVolume): ChunkVolume {
  volume.sealed = true;
  for (const band of volume.bands) sort_sub_voxels(band.extra);
  return volume;
}

/** Insertion sort by column — the list is a handful of entries when it is
 * anything at all — moving every lane together. */
function sort_sub_voxels(extra: SubVoxels): void {
  const n = extra.col.len;
  const col = extra.col.mem, z = extra.z.mem, material = extra.material.mem, layer = extra.layer_idx.mem, sub = extra.sub.mem;
  for (let i = 1; i < n; i++) {
    const c = col[i], zi = z[i], m = material[i], l = layer[i], s = sub[i];
    let j = i - 1;
    while (j >= 0 && col[j] > c) {
      col[j + 1] = col[j];
      z[j + 1] = z[j];
      material[j + 1] = material[j];
      layer[j + 1] = layer[j];
      sub[j + 1] = sub[j];
      j--;
    }
    col[j + 1] = c;
    z[j + 1] = zi;
    material[j + 1] = m;
    layer[j + 1] = l;
    sub[j + 1] = s;
  }
}

/** The range [lo, hi) of a band's extra sub-voxels in column `col`, once
 * the volume is sealed; [0, 0) when it has none there. */
export function sub_voxel_range(extra: SubVoxels, col: u32): [u32, u32] {
  const cols = extra.col.mem;
  let lo = 0, hi = extra.col.len;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (cols[mid] < col) lo = mid + 1;
    else hi = mid;
  }
  let end = lo;
  while (end < extra.col.len && cols[end] === col) end++;
  return [lo, end];
}

function get_or_create_band(volume: ChunkVolume, band_z: i32): VoxelBand {
  if (volume.sealed) throw new Error("render: cannot mutate a sealed chunk volume");
  let insert_at = 0;
  while (insert_at < volume.bands.length && volume.bands[insert_at].band_z < band_z) insert_at++;
  const existing = volume.bands[insert_at];
  if (existing?.band_z === band_z) return existing;

  const pooled = band_pool.pop();
  if (pooled) band_pool_stats.reused++;
  else band_pool_stats.allocated++;
  // Filled below either way: a pool block keeps whatever its last borrower
  // wrote (core_pool.ts's quick-format contract).
  const band = pooled ?? new_band(band_z);
  band.band_z = band_z;
  band.material.fill(0);
  band.layer_idx.fill(NO_LAYER);
  drop_sub_cell(band);
  clear_sub_voxels(band.extra);
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

  // A later layer replacing the voxel takes its overflow with it.
  const extra = band.extra;
  if (extra.col.len > 0) {
    const col = (cy & L2_BAND_CELL_MASK) * L2_BAND_CELL_SPAN + (cx & L2_BAND_CELL_MASK);
    let w = 0;
    for (let i = 0; i < extra.col.len; i++) {
      if (extra.col.mem[i] === col && extra.z.mem[i] === z) continue;
      extra.col.mem[w] = extra.col.mem[i];
      extra.z.mem[w] = extra.z.mem[i];
      extra.material.mem[w] = extra.material.mem[i];
      extra.layer_idx.mem[w] = extra.layer_idx.mem[i];
      extra.sub.mem[w] = extra.sub.mem[i];
      w++;
    }
    extra.col.truncate(w);
    extra.z.truncate(w);
    extra.material.truncate(w);
    extra.layer_idx.truncate(w);
    extra.sub.truncate(w);
  }

  if (sub_cell) {
    if (!band.sub_cell) {
      const lazy = borrow((b, o, n) => new Uint8Array(b, o, n), L2_BAND_VOXEL_CNT * 4, 1);
      band.blocks.sub_cell = lazy.block;
      // Zeroed by hand: a typed array of its own started zeroed, a pool
      // block does not, and zero is SUB_CELL_NONE.
      band.sub_cell = lazy.view.fill(0);
    }
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
export function append_chunk_sub_voxel(volume: ChunkVolume, cx: u32, cy: u32, z: i32, material: VoxelMaterial, sub: u32): void {
  const loc = locate_voxel(cx, cy, z);
  const band = get_or_create_band(volume, loc.band_z);
  const extra = band.extra;
  extra.col.push((cy & L2_BAND_CELL_MASK) * L2_BAND_CELL_SPAN + (cx & L2_BAND_CELL_MASK));
  extra.z.push(z);
  extra.material.push(pack_material(material));
  extra.layer_idx.push(material.layer_idx);
  extra.sub.push(sub);
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
