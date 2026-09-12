// @pxl/web/render/layer/pipeline/voxel_scan.ts

/** Shared band scan used by rendering and raycasting. */

import type { i32, u32 } from "@xpute/core/kit/word.ts";
import type { Block } from "@xpute/core/ds/buddy.ts";
import { U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import type { V3 } from "@pxl/shared/spec/space/v3.ts";

import { Vector } from "@xpute/core/ds/arena.ts";

import { CORE_ALLOC, CORE_POOL } from "./core_pool.ts";

import {
  band_voxel_idx,
  type ChunkVolume,
  get_chunk_band,
  L2_BAND_CELL_MASK,
  L2_BAND_CELL_SPAN,
  MaterialClass,
  ordered_chunk_bands,
  SUB_CELL_NONE,
  type SubVoxelEntry,
  unpack_sub_cell_fraction,
  type VoxelBand,
} from "./volume.ts";

export type MeshRun = {
  z0: i32;
  z1: i32;
  r: u32;
  g: u32;
  b: u32;
  layer_idx: u32;
  material_class: MaterialClass;
  sub_sz: u32;
  sub_ox: u32;
  sub_oy: u32;
  sub_oz: u32;
};

function push_sub_voxel_run(sink: RunSink, entry: SubVoxelEntry): void {
  sink.push(entry.z, entry.z, entry.r, entry.g, entry.b, entry.layer_idx, entry.material_class, entry.sub_sz, entry.sub_ox, entry.sub_oy, entry.sub_oz);
}

/**
 * Per-(band, column) buckets of `extra_sub_voxels`, built once and reused
 * across every column `merge_column_runs` visits for the same chunk
 * (`scan_all_column_runs` below) — filtering a band's `extra_sub_voxels` by
 * `cx`/`cy` fresh on every one of those 4096 calls would be an O(4096 * N)
 * re-scan of the same array, dominating cost on a chunk with many placed
 * sub-cell objects (large N).
 */
export type SubVoxelIndex = ReadonlyMap<VoxelBand, ReadonlyMap<u32, readonly SubVoxelEntry[]>>;

export function index_extra_sub_voxels(bands: readonly VoxelBand[]): SubVoxelIndex {
  const index = new Map<VoxelBand, Map<u32, SubVoxelEntry[]>>();
  for (const band of bands) {
    if (band.extra_sub_voxels.length === 0) continue;
    const per_column = new Map<u32, SubVoxelEntry[]>();
    for (const entry of band.extra_sub_voxels) {
      const key = entry.cy * L2_BAND_CELL_SPAN + entry.cx;
      let list = per_column.get(key);
      if (!list) per_column.set(key, list = []);
      list.push(entry);
    }
    index.set(band, per_column);
  }
  return index;
}

/**
 * Where one column's runs go. Two implementations: an array of MeshRun
 * objects for the raycast path, and the packed spans of a ColumnRunSet for
 * the mesher's whole-chunk scan.
 *
 * `push` returns the index it wrote so `extend` can raise that run's top
 * afterwards, which is how a run grows without being re-emitted. Emitting at
 * *open* rather than at close is load-bearing: it keeps a column's runs in
 * the order the walk finds them, and chunk_mesher.ts's neighbour scans stop
 * early on the first run that starts above the range they care about. A run
 * flushed when it closed would land after the sub-voxel runs of every band
 * it spans, putting a solid run behind a higher one and quietly cutting
 * those scans short.
 *
 * (Sub-voxel runs already break strict ascending order — a band appends its
 * own after that band's voxel loop — but they are the one thing those scans
 * skip unconditionally, so they can never be the run a scan stops on.)
 */
type RunSink = {
  push(
    z0: i32,
    z1: i32,
    r: u32,
    g: u32,
    b: u32,
    layer_idx: u32,
    material_class: MaterialClass,
    sub_sz: u32,
    sub_ox: u32,
    sub_oy: u32,
    sub_oz: u32,
  ): u32;
  extend(index: u32, z1: i32): void;
};

/**
 * The band walk itself, shared by both sinks.
 *
 * The open run lives in locals rather than in the sink, because the merge
 * test compares against its material and the sink may have written that
 * somewhere it cannot cheaply read back.
 */
function walk_column_runs(
  volume: ChunkVolume,
  cx: u32,
  cy: u32,
  only_band_z: i32 | undefined,
  sub_voxel_index: SubVoxelIndex | undefined,
  sink: RunSink,
): void {
  if (volume.bands.length === 0) return;

  const band_x = Math.floor(cx / L2_BAND_CELL_SPAN);
  const band_y = Math.floor(cy / L2_BAND_CELL_SPAN);
  const local_x = cx & L2_BAND_CELL_MASK;
  const local_y = cy & L2_BAND_CELL_MASK;

  let open = -1;
  let open_r = 0, open_g = 0, open_b = 0, open_layer = 0, open_class = MaterialClass.LAND;

  const ordered = only_band_z === undefined ? ordered_chunk_bands(volume) : (() => {
    const band = get_chunk_band(volume, band_x, band_y, only_band_z);
    return band ? [band] : [];
  })();
  let previous_band_z: i32 | null = null;
  for (const band of ordered) {
    const band_z = band.band_z;
    // Empty signed-Z bands break a material run. Adjacent bands do not:
    // buried faces at a cache boundary must not reappear in the mesh.
    if (previous_band_z !== null && band_z !== previous_band_z + 1) open = -1;
    previous_band_z = band_z;

    for (let local_z = 0; local_z < L2_BAND_CELL_SPAN; local_z++) {
      const idx = band_voxel_idx(local_x, local_y, local_z);
      const packed = band.material[idx];
      if (packed === 0) {
        open = -1;
        continue;
      }

      const z = band_z * L2_BAND_CELL_SPAN + local_z;
      const r = (packed >>> 16) & 255, g = (packed >>> 8) & 255, b = packed & 255;
      const material_class = (packed >>> 24) as MaterialClass;
      const layer_idx = band.layer_idx[idx];
      const so = idx * 4;
      const sub_sz = band.sub_cell?.[so + 3] ?? SUB_CELL_NONE;

      if (sub_sz !== SUB_CELL_NONE) {
        sink.push(z, z, r, g, b, layer_idx, material_class, sub_sz, band.sub_cell![so], band.sub_cell![so + 1], band.sub_cell![so + 2]);
        open = -1;
      } else if (
        open >= 0 && material_class !== MaterialClass.OBJECT && open_r === r && open_g === g && open_b === b &&
        open_layer === layer_idx && open_class === material_class
      ) {
        sink.extend(open, z);
      } else {
        open = sink.push(z, z, r, g, b, layer_idx, material_class, SUB_CELL_NONE, 0, 0, 0);
        open_r = r;
        open_g = g;
        open_b = b;
        open_layer = layer_idx;
        open_class = material_class;
      }
    }

    if (sub_voxel_index) {
      const entries = sub_voxel_index.get(band)?.get(cy * L2_BAND_CELL_SPAN + cx);
      if (entries) { for (const entry of entries) push_sub_voxel_run(sink, entry); }
    } else {
      for (const entry of band.extra_sub_voxels) {
        if (entry.cx !== cx || entry.cy !== cy) continue;
        push_sub_voxel_run(sink, entry);
      }
    }
  }
}

class ArrayRunSink implements RunSink {
  readonly runs: MeshRun[] = [];

  push(
    z0: i32,
    z1: i32,
    r: u32,
    g: u32,
    b: u32,
    layer_idx: u32,
    material_class: MaterialClass,
    sub_sz: u32,
    sub_ox: u32,
    sub_oy: u32,
    sub_oz: u32,
  ): u32 {
    return this.runs.push({ z0, z1, r, g, b, layer_idx, material_class, sub_sz, sub_ox, sub_oy, sub_oz }) - 1;
  }

  extend(index: u32, z1: i32): void {
    this.runs[index].z1 = z1;
  }
}

/**
 * One column's runs as objects. Still the raycast's entry point, where it
 * runs for one column on a hover and the result is read once and dropped —
 * the whole-chunk scan below writes into typed spans instead.
 */
export function merge_column_runs(
  volume: ChunkVolume,
  cx: u32,
  cy: u32,
  only_band_z?: i32,
  sub_voxel_index?: SubVoxelIndex,
): MeshRun[] {
  const sink = new ArrayRunSink();
  walk_column_runs(volume, cx, cy, only_band_z, sub_voxel_index, sink);
  return sink.runs;
}

export function topmost_voxel(
  volume: ChunkVolume,
  cx: u32,
  cy: u32,
  only_band_z?: i32,
): { z: i32; r: u32; g: u32; b: u32; layer_idx: u32 } | null {
  if (volume.bands.length === 0) return null;
  const band_x = Math.floor(cx / L2_BAND_CELL_SPAN);
  const band_y = Math.floor(cy / L2_BAND_CELL_SPAN);
  const local_x = cx & L2_BAND_CELL_MASK;
  const local_y = cy & L2_BAND_CELL_MASK;

  if (only_band_z !== undefined) {
    const band = get_chunk_band(volume, band_x, band_y, only_band_z);
    return band ? topmost_in_band(band, local_x, local_y, only_band_z) : null;
  }
  const ordered = ordered_chunk_bands(volume);
  for (let i = ordered.length - 1; i >= 0; i--) {
    const band = ordered[i];
    const band_z = band.band_z;
    const top = topmost_in_band(band, local_x, local_y, band_z);
    if (top) return top;
  }
  return null;
}

function topmost_in_band(
  band: VoxelBand,
  local_x: u32,
  local_y: u32,
  band_z: i32,
): { z: i32; r: u32; g: u32; b: u32; layer_idx: u32 } | null {
  for (let local_z = L2_BAND_CELL_SPAN - 1; local_z >= 0; local_z--) {
    const idx = band_voxel_idx(local_x, local_y, local_z);
    const packed = band.material[idx];
    if (packed === 0) continue;
    return {
      z: band_z * L2_BAND_CELL_SPAN + local_z,
      r: (packed >>> 16) & 255,
      g: (packed >>> 8) & 255,
      b: packed & 255,
      layer_idx: band.layer_idx[idx],
    };
  }
  return null;
}

/**
 * Every column's runs, scanned once. `merge_column_runs` walks live band
 * data (`ChunkVolume`), which the renderer only keeps alive for one
 * synchronous composite pass — this copies each column's result out into
 * plain `MeshRun` objects (already true of `merge_column_runs`'s own
 * output, never a view into the volume's typed arrays) so callers can
 * retain and reuse `MeshRun[][]` across LOD rebuilds long after that
 * volume has been released back to its pool.
 */
/** Columns in one chunk — the same 64x64 the mesher indexes with CHUNK_CELL_SPAN. */
const COLUMN_CNT: u32 = L2_BAND_CELL_SPAN * L2_BAND_CELL_SPAN;

/**
 * Every column's runs for one chunk, laid out as parallel arrays over a
 * single pooled block.
 *
 * This was `MeshRun[][]`: 4096 arrays plus an object per run, ~10k objects
 * and about a megabyte for a dense chunk, *retained* for as long as the
 * chunk keeps its scan. Measured, it was the largest remaining allocator
 * once the vertex blocks were pooled — a committing frame moved +1.43MB
 * with nothing else left to account for it.
 *
 * A run is 17 bytes here against roughly a hundred as an object, and one
 * allocation replaces ten thousand. The fields are split by width rather
 * than interleaved because that is what makes them typed-array spans at all;
 * nothing reads a whole run at once anyway.
 *
 * `start` is a prefix index: column c's runs are `[start[c], start[c + 1])`,
 * which is what lets the whole thing be flat — a column's runs are
 * contiguous because merge_column_runs finishes one column before the next.
 */
export type ColumnRunSet = {
  block: Block;
  /** Total runs across every column. */
  count: u32;
  /** COLUMN_CNT + 1 offsets into the per-run spans below. */
  start: Uint32Array;
  z0: Int32Array;
  z1: Int32Array;
  r: Uint8Array;
  g: Uint8Array;
  b: Uint8Array;
  /**
   * Four texture-array layers packed one per byte, which the shader averages
   * at 25% each.
   *
   * Four rather than one because a coarse tier's cell can span several
   * materials and showing only the first reads as blotchy: the taps are how
   * a merged cell blends grass into road into roof. A fine run has one
   * material, so it writes the same layer into all four and the average is
   * that layer — which is what the finest tier has always done.
   */
  layer_taps: Uint32Array;
  material_class: Uint8Array;
  /**
   * SUB_CELL_NONE (0) for a full-cell run; otherwise the run's box is inset
   * inside its cell and `sub_sx`/`sub_sy`/`sub_sz` give its extent per axis.
   *
   * `sub_sz` is the discriminator for all three, and it scales the run's
   * whole z extent rather than one cell — for a sub-voxel object the run is
   * one cell tall so the two agree, and for a coarse LOD cell it is what
   * lets the box keep an exact multi-cell height while shrinking in XY.
   *
   * Per-axis rather than one isotropic size because sparsity has a
   * direction: a lone pillar under a coarse cell is narrow in XY and full
   * height, and a single scale factor can only say one of those two things.
   */
  sub_sz: Uint8Array;
  sub_sx: Uint8Array;
  sub_sy: Uint8Array;
  sub_ox: Uint8Array;
  sub_oy: Uint8Array;
  sub_oz: Uint8Array;
};

/** The prefix index, filled during the walk and copied into the block. */
const COLUMN_STARTS = CORE_ALLOC.take(Uint32Array, COLUMN_CNT + 1);

const RUN_I32_FIELDS: u32 = 3; // z0, z1, layer_taps
const RUN_U8_FIELDS: u32 = 10; // r g b material_class sub_sz sub_sx sub_sy sub_ox sub_oy sub_oz

export function alloc_column_run_set(count: u32): ColumnRunSet {
  const block = CORE_POOL.acquire((COLUMN_CNT + 1) * 4 + count * (RUN_I32_FIELDS * 4 + RUN_U8_FIELDS));
  const buf = block.buf;
  let off = block.byte_offset;
  const start = new Uint32Array(buf, off, COLUMN_CNT + 1);
  off += (COLUMN_CNT + 1) * 4;
  const i32 = (): Int32Array => {
    const view = new Int32Array(buf, off, count);
    off += count * 4;
    return view;
  };
  const z0 = i32(), z1 = i32();
  const layer_taps = new Uint32Array(buf, off, count);
  off += count * 4;
  const u8 = (): Uint8Array => {
    const view = new Uint8Array(buf, off, count);
    off += count;
    return view;
  };
  return {
    block,
    count,
    start,
    z0,
    z1,
    layer_taps,
    r: u8(),
    g: u8(),
    b: u8(),
    material_class: u8(),
    sub_sz: u8(),
    sub_sx: u8(),
    sub_sy: u8(),
    sub_ox: u8(),
    sub_oy: u8(),
    sub_oz: u8(),
  };
}

/** Hands the set's block back. Every span above is dead afterwards. */
export function release_column_runs(set: ColumnRunSet): void {
  CORE_POOL.release(set.block);
}

/**
 * Scratch the whole-chunk scan writes into before it knows how many runs
 * there are.
 *
 * The block has to be sized exactly, and the count is only known once the
 * walk is done, so there is one buffering step either way. What this avoids
 * is the *other* thing the old shape did: 4096 arrays and an object per run,
 * roughly a megabyte for a dense chunk, allocated and thrown away on every
 * scan. These grow to a high-water mark and are reused for the rest of the
 * session, and the copy out of them is eleven typed-array memcpys rather
 * than a field-by-field loop over ten thousand objects.
 */
const SCRATCH_CAP = { init_cap: 1 << 13 };
const SC_Z0 = new Vector(Int32Array, { ...SCRATCH_CAP, alloc: CORE_ALLOC });
const SC_Z1 = new Vector(Int32Array, { ...SCRATCH_CAP, alloc: CORE_ALLOC });
const SC_R = new Vector(Uint8Array, { ...SCRATCH_CAP, alloc: CORE_ALLOC });
const SC_G = new Vector(Uint8Array, { ...SCRATCH_CAP, alloc: CORE_ALLOC });
const SC_B = new Vector(Uint8Array, { ...SCRATCH_CAP, alloc: CORE_ALLOC });
const SC_LAYER = new Vector(Uint32Array, { ...SCRATCH_CAP, alloc: CORE_ALLOC });
const SC_CLASS = new Vector(Uint8Array, { ...SCRATCH_CAP, alloc: CORE_ALLOC });
const SC_SUB_SZ = new Vector(Uint8Array, { ...SCRATCH_CAP, alloc: CORE_ALLOC });
const SC_SUB_OX = new Vector(Uint8Array, { ...SCRATCH_CAP, alloc: CORE_ALLOC });
const SC_SUB_OY = new Vector(Uint8Array, { ...SCRATCH_CAP, alloc: CORE_ALLOC });
const SC_SUB_OZ = new Vector(Uint8Array, { ...SCRATCH_CAP, alloc: CORE_ALLOC });

const SPAN_SINK: RunSink = {
  push(z0, z1, r, g, b, layer_idx, material_class, sub_sz, sub_ox, sub_oy, sub_oz) {
    const i = SC_Z0.push(z0);
    SC_Z1.push(z1);
    SC_R.push(r);
    SC_G.push(g);
    SC_B.push(b);
    SC_LAYER.push(layer_idx * 0x01010101);
    SC_CLASS.push(material_class);
    SC_SUB_SZ.push(sub_sz);
    SC_SUB_OX.push(sub_ox);
    SC_SUB_OY.push(sub_oy);
    SC_SUB_OZ.push(sub_oz);
    return i;
  },
  extend(index, z1) {
    SC_Z1.set(index, z1);
  },
};

/**
 * A chunk scan in progress: the composite it reads, and the next row of
 * columns to walk.
 *
 * The scan is column-major and a run can span bands, so the unit that can be
 * interrupted is a row of columns rather than a band — a row keeps every run
 * whole and appends to the scratch spans in exactly the order the packed set
 * wants, so finishing is a copy and nothing has to be stitched.
 *
 * One at a time. The scratch spans are module-level and there is one set of
 * them, so `begin_column_scan` refuses a second scan while one is open, and
 * the holder is expected to finish or abort it. That is also what bounds
 * how many composites are held alive across frames: one.
 */
export type ColumnScan = {
  volume: ChunkVolume;
  sub_voxel_index: SubVoxelIndex;
  next_row: u32;
};

let open_scan: ColumnScan | null = null;

/** Whether a scan is open — so a would-be holder can skip composing. */
export function column_scan_open(): boolean {
  return open_scan !== null;
}

/** Rows a step walks. Sized so a step on a band-heavy chunk stays around a
 * millisecond: the whole 64-row scan was measured at up to 15 ms. */
export const SCAN_ROWS_PER_STEP: u32 = 8;

/** Opens a scan over `volume`, or returns null while another is open. */
export function begin_column_scan(volume: ChunkVolume): ColumnScan | null {
  if (open_scan !== null) return null;
  SC_Z0.truncate(0);
  SC_Z1.truncate(0);
  SC_R.truncate(0);
  SC_G.truncate(0);
  SC_B.truncate(0);
  SC_LAYER.truncate(0);
  SC_CLASS.truncate(0);
  SC_SUB_SZ.truncate(0);
  SC_SUB_OX.truncate(0);
  SC_SUB_OY.truncate(0);
  SC_SUB_OZ.truncate(0);
  open_scan = { volume, sub_voxel_index: index_extra_sub_voxels(volume.bands), next_row: 0 };
  return open_scan;
}

/** Walks up to `rows` more rows of columns. Returns true once every row is
 * done, at which point `finish_column_scan` packs the result. */
export function scan_column_rows(scan: ColumnScan, rows: u32 = SCAN_ROWS_PER_STEP): boolean {
  const starts = COLUMN_STARTS;
  const end = Math.min(L2_BAND_CELL_SPAN, scan.next_row + rows);
  for (let cy: u32 = scan.next_row; cy < end; cy++) {
    for (let cx: u32 = 0; cx < L2_BAND_CELL_SPAN; cx++) {
      starts[cy * L2_BAND_CELL_SPAN + cx] = SC_Z0.len;
      walk_column_runs(scan.volume, cx, cy, undefined, scan.sub_voxel_index, SPAN_SINK);
    }
  }
  scan.next_row = end;
  return end >= L2_BAND_CELL_SPAN;
}

/** Packs a finished scan into an exactly-sized set and closes it. */
export function finish_column_scan(scan: ColumnScan): ColumnRunSet {
  if (scan !== open_scan || scan.next_row < L2_BAND_CELL_SPAN) {
    throw new Error("render: finish_column_scan on a scan that is not the open, complete one");
  }
  const starts = COLUMN_STARTS;
  starts[COLUMN_CNT] = SC_Z0.len;

  const set = alloc_column_run_set(SC_Z0.len);
  set.start.set(starts);
  const n = SC_Z0.len;
  set.z0.set(SC_Z0.mem.subarray(0, n));
  set.z1.set(SC_Z1.mem.subarray(0, n));
  set.r.set(SC_R.mem.subarray(0, n));
  set.g.set(SC_G.mem.subarray(0, n));
  set.b.set(SC_B.mem.subarray(0, n));
  set.layer_taps.set(SC_LAYER.mem.subarray(0, n));
  set.material_class.set(SC_CLASS.mem.subarray(0, n));
  // A baked sub-voxel object is a cube, so its three extents are one value.
  // Only the coarse mesher writes them apart.
  set.sub_sz.set(SC_SUB_SZ.mem.subarray(0, n));
  set.sub_sx.set(SC_SUB_SZ.mem.subarray(0, n));
  set.sub_sy.set(SC_SUB_SZ.mem.subarray(0, n));
  set.sub_ox.set(SC_SUB_OX.mem.subarray(0, n));
  set.sub_oy.set(SC_SUB_OY.mem.subarray(0, n));
  set.sub_oz.set(SC_SUB_OZ.mem.subarray(0, n));
  open_scan = null;
  return set;
}

/** Drops a scan without packing it — the holder went away, or its source
 * changed under it. The scratch is simply reused by the next `begin`. */
export function abort_column_scan(scan: ColumnScan): void {
  if (scan === open_scan) open_scan = null;
}

/**
 * Scans a whole chunk in one call and packs the result — the three steps
 * above, run to completion. What the tests and the raycast-free callers
 * want; the LOD commit path steps through them across frames instead.
 *
 * Nothing here allocates per run: the walk writes straight into the scratch
 * spans above and the block is filled from them by copy. merge_column_runs
 * keeps its object output for the raycast, which asks for one column at a
 * time and drops the answer.
 */
export function scan_all_column_runs(volume: ChunkVolume): ColumnRunSet {
  const scan = begin_column_scan(volume);
  if (!scan) throw new Error("render: scan_all_column_runs while a stepped scan is open");
  scan_column_rows(scan, L2_BAND_CELL_SPAN);
  return finish_column_scan(scan);
}

/**
 * Packs a column-per-entry array of runs into a run set.
 *
 * Not on the scan's own path — that writes into the spans directly — but a
 * test can build a chunk's runs by hand and get the same packed shape the
 * mesher consumes, without a ChunkVolume it does not care about.
 */
export function pack_column_runs(per_column: readonly (readonly MeshRun[])[]): ColumnRunSet {
  let count: u32 = 0;
  for (const runs of per_column) count += runs.length;

  const set = alloc_column_run_set(count);
  let w: u32 = 0;
  for (let c: u32 = 0; c < COLUMN_CNT; c++) {
    set.start[c] = w;
    for (const run of per_column[c] ?? []) {
      set.z0[w] = run.z0;
      set.z1[w] = run.z1;
      set.r[w] = run.r;
      set.g[w] = run.g;
      set.b[w] = run.b;
      set.layer_taps[w] = run.layer_idx * 0x01010101;
      set.material_class[w] = run.material_class;
      set.sub_sz[w] = run.sub_sz;
      set.sub_sx[w] = run.sub_sz;
      set.sub_sy[w] = run.sub_sz;
      set.sub_ox[w] = run.sub_ox;
      set.sub_oy[w] = run.sub_oy;
      set.sub_oz[w] = run.sub_oz;
      w++;
    }
  }
  set.start[COLUMN_CNT] = w;
  return set;
}

export type RunBox = { min: V3; max: V3 };

export function run_to_box(cx: u32, cy: u32, run: MeshRun): RunBox {
  if (run.sub_sz === SUB_CELL_NONE) {
    return {
      min: [cx * U_PER_CELL, cy * U_PER_CELL, run.z0 * U_PER_CELL],
      max: [(cx + 1) * U_PER_CELL, (cy + 1) * U_PER_CELL, (run.z1 + 1) * U_PER_CELL],
    };
  }

  const sz = unpack_sub_cell_fraction(run.sub_sz) * U_PER_CELL;
  const min: V3 = [
    (cx + unpack_sub_cell_fraction(run.sub_ox)) * U_PER_CELL,
    (cy + unpack_sub_cell_fraction(run.sub_oy)) * U_PER_CELL,
    (run.z0 + unpack_sub_cell_fraction(run.sub_oz)) * U_PER_CELL,
  ];
  return { min, max: [min[0] + sz, min[1] + sz, min[2] + sz] };
}
