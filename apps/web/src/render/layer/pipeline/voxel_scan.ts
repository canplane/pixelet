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
  sub_voxel_range,
  type SubVoxels,
  unpack_sub_cell_fraction,
  type VoxelBand,
  type VoxelMaterial,
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

/** One band's overflow sub-voxels in a column, pushed as runs of height
 * one — the lanes volume.ts sorted by column when the volume was sealed. */
function push_sub_voxel_runs(sink: RunSink, extra: SubVoxels, col: u32): void {
  const [lo, hi] = sub_voxel_range(extra, col);
  for (let i = lo; i < hi; i++) {
    const z = extra.z.mem[i];
    const m = extra.material.mem[i];
    const sub = extra.sub.mem[i];
    sink.push(z, z, (m >>> 16) & 255, (m >>> 8) & 255, m & 255, extra.layer_idx.mem[i], (m >>> 24) as MaterialClass, sub >>> 24, sub & 255, (sub >>> 8) & 255, (sub >>> 16) & 255);
  }
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

    if (band.extra.col.len > 0) push_sub_voxel_runs(sink, band.extra, local_y * L2_BAND_CELL_SPAN + local_x);
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
): MeshRun[] {
  const sink = new ArrayRunSink();
  walk_column_runs(volume, cx, cy, only_band_z, sink);
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
/**
 * The widest unit a scan can produce, in chunks on a side. A coarse tier
 * works on a block of chunks rather than one — step 16 on 4×4 — so that the
 * cost per unit stays flat across tiers rather than growing sixteenfold at
 * the tier that draws least (discussions/20260912-coarse-tiers-need-a-larger-unit.md).
 * The scratch below is sized for this; a scan may be narrower.
 */
export const MAX_UNIT_CHUNKS: u32 = 4;
const MAX_SPAN: u32 = L2_BAND_CELL_SPAN * MAX_UNIT_CHUNKS;
const MAX_COLUMN_CNT: u32 = MAX_SPAN * MAX_SPAN;

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
  /** Columns on a side: 64 for one chunk, 256 for a 4×4 block. Column
   * `(cx, cy)` is `cy * span + cx`, which is how the mesher reads it. */
  span: u32;
  /** span² + 1 offsets into the per-run spans below. */
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
const COLUMN_STARTS = CORE_ALLOC.take(Uint32Array, MAX_COLUMN_CNT + 1);

const RUN_I32_FIELDS: u32 = 3; // z0, z1, layer_taps
const RUN_U8_FIELDS: u32 = 10; // r g b material_class sub_sz sub_sx sub_sy sub_ox sub_oy sub_oz

export function alloc_column_run_set(count: u32, span: u32 = L2_BAND_CELL_SPAN): ColumnRunSet {
  const column_cnt = span * span;
  const block = CORE_POOL.acquire((column_cnt + 1) * 4 + count * (RUN_I32_FIELDS * 4 + RUN_U8_FIELDS));
  const buf = block.buf;
  let off = block.byte_offset;
  const start = new Uint32Array(buf, off, column_cnt + 1);
  off += (column_cnt + 1) * 4;
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
    span,
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
  /** Columns on a side of the whole unit. */
  span: u32;
  /**
   * The chunk-row being walked: one composite per chunk across the unit,
   * null where a chunk has nothing to draw. The prefix index is over the
   * unit's columns in row-major order, and a unit row crosses every chunk of
   * the chunk-row, so a row cannot be walked until all of them are here —
   * which is why the unit is fed a chunk-row at a time rather than a chunk.
   * A one-chunk scan is a chunk-row of one.
   */
  row_volumes: (ChunkVolume | null)[];
  /** Which chunk-row `row_volumes` holds, 0..span / L2_BAND_CELL_SPAN. */
  chunk_row: u32;
  /** Next row within the chunk-row to walk, 0..L2_BAND_CELL_SPAN. */
  next_row: u32;
  /** Whether `row_volumes` has been handed over for the current chunk-row. */
  row_fed: boolean;
};

let open_scan: ColumnScan | null = null;

/** Whether a scan is open — so a would-be holder can skip composing. */
export function column_scan_open(): boolean {
  return open_scan !== null;
}

/** Rows a step walks. Sized so a step on a band-heavy chunk stays around a
 * millisecond: the whole 64-row scan was measured at up to 15 ms. */
export const SCAN_ROWS_PER_STEP: u32 = 8;

/**
 * Opens a scan of a `span`-wide unit, or returns null while another is open.
 *
 * A unit wider than a chunk is not composited whole — a 4×4 block would be
 * sixteen 64³ bands per Z band, 21 MB — but a chunk-row at a time: the
 * composites of one row of chunks are walked together, unit row by unit row,
 * into the unit's run set, so the run set is `span`-wide while at most one
 * chunk-row of volumes is alive. `first_row` is the first chunk-row, one
 * composite per chunk left to right, null for a chunk with nothing to draw;
 * a block scan feeds the remaining chunk-rows through `scan_next_row`.
 */
export function begin_column_scan(first_row: ChunkVolume | readonly (ChunkVolume | null)[], span: u32 = L2_BAND_CELL_SPAN): ColumnScan | null {
  if (open_scan !== null) return null;
  if (span % L2_BAND_CELL_SPAN !== 0 || span > MAX_SPAN) throw new Error(`render: a scan unit is a whole number of chunks up to ${MAX_UNIT_CHUNKS} on a side, not ${span}`);
  const row = Array.isArray(first_row) ? first_row as readonly (ChunkVolume | null)[] : [first_row as ChunkVolume];
  if (row.length !== span / L2_BAND_CELL_SPAN) throw new Error(`render: a chunk-row of a ${span}-wide unit holds ${span / L2_BAND_CELL_SPAN} chunks, not ${row.length}`);
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
  open_scan = {
    span,
    row_volumes: [...row],
    chunk_row: 0,
    next_row: 0,
    row_fed: true,
  };
  return open_scan;
}

/** Chunks on a side of the unit this scan covers. */
export function scan_unit_chunks(scan: ColumnScan): u32 {
  return scan.span / L2_BAND_CELL_SPAN;
}

/**
 * Hands a block scan its next chunk-row of composites, once the previous
 * chunk-row is fully walked. The previous composites are the caller's to
 * release; the scan keeps no reference to them past this call.
 */
export function scan_next_row(scan: ColumnScan, row: readonly (ChunkVolume | null)[]): void {
  if (scan !== open_scan) throw new Error("render: scan_next_row on a scan that is not the open one");
  if (!scan_wants_row(scan)) throw new Error("render: scan_next_row before the current chunk-row is walked");
  if (row.length !== scan_unit_chunks(scan)) throw new Error(`render: a chunk-row holds ${scan_unit_chunks(scan)} chunks, not ${row.length}`);
  scan.row_volumes = [...row];
  scan.chunk_row++;
  scan.next_row = 0;
  scan.row_fed = true;
}

/** Whether the scan is waiting for its next chunk-row, as opposed to
 * mid-row or complete. */
export function scan_wants_row(scan: ColumnScan): boolean {
  return !scan.row_fed && scan.chunk_row + 1 < scan_unit_chunks(scan);
}

/** Whether every chunk-row has been walked. */
export function scan_complete(scan: ColumnScan): boolean {
  return !scan.row_fed && scan.chunk_row + 1 >= scan_unit_chunks(scan);
}

/**
 * Walks up to `rows` more unit rows of the current chunk-row. Returns true
 * once the whole unit is walked — every chunk-row fed and every row of it —
 * at which point `finish_column_scan` packs the result. For a block, a true
 * return from `scan_wants_row` afterwards means "this chunk-row is done,
 * feed the next".
 */
export function scan_column_rows(scan: ColumnScan, rows: u32 = SCAN_ROWS_PER_STEP): boolean {
  if (!scan.row_fed) return scan_complete(scan);
  const starts = COLUMN_STARTS;
  const { span, row_volumes } = scan;
  const n = scan_unit_chunks(scan);
  const cy0 = scan.chunk_row * L2_BAND_CELL_SPAN;
  const end = Math.min(L2_BAND_CELL_SPAN, scan.next_row + rows);
  for (let cy: u32 = scan.next_row; cy < end; cy++) {
    for (let ux: u32 = 0; ux < n; ux++) {
      const volume = row_volumes[ux];
      const cx0 = ux * L2_BAND_CELL_SPAN;
      const row_base = (cy0 + cy) * span + cx0;
      if (volume === null) {
        // Nothing to draw here: the prefix still has to advance over it.
        for (let cx: u32 = 0; cx < L2_BAND_CELL_SPAN; cx++) starts[row_base + cx] = SC_Z0.len;
        continue;
      }
      for (let cx: u32 = 0; cx < L2_BAND_CELL_SPAN; cx++) {
        starts[row_base + cx] = SC_Z0.len;
        walk_column_runs(volume, cx, cy, undefined, SPAN_SINK);
      }
    }
  }
  scan.next_row = end;
  if (end >= L2_BAND_CELL_SPAN) {
    scan.row_fed = false;
    scan.row_volumes = [];
  }
  return scan_complete(scan);
}

/** Packs a finished scan into an exactly-sized set and closes it. */
export function finish_column_scan(scan: ColumnScan): ColumnRunSet {
  if (scan !== open_scan || !scan_complete(scan)) {
    throw new Error("render: finish_column_scan on a scan that is not the open, complete one");
  }
  const column_cnt = scan.span * scan.span;
  COLUMN_STARTS[column_cnt] = SC_Z0.len;
  // A baked sub-voxel object is a cube, so its three extents are one value;
  // only the coarse mesher writes them apart. pack_scratch copies sub_sz into
  // all three.
  const set = pack_scratch(scan.span);
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
 * What a layer writes its column runs into on the direct path: one call per
 * run, columns in row-major order, each column's runs together. Typed
 * scratch underneath — no run objects are made.
 */
export type ColumnRunWriter = {
  /** Starts column `col` (row-major, `cy * span + cx`). Columns must be
   * started in order; a column with no runs is still started. */
  column(col: u32): void;
  run(z0: i32, z1: i32, m: VoxelMaterial): void;
};

/**
 * Builds a run set directly from a writer, without a volume or a scan: the
 * path a coarse tier takes when its layers can say their content as plain
 * column runs (WorldLayer.runs). Same scratch, same packing as a scan, so
 * the result is what a scan of the equivalent volume would have produced.
 * Refuses to run while a stepped scan is open, since the scratch is shared.
 */
export function fill_column_runs(span: u32, fill: (w: ColumnRunWriter) => void): ColumnRunSet {
  if (open_scan !== null) throw new Error("render: fill_column_runs while a stepped scan is open");
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
  const starts = COLUMN_STARTS;
  const column_cnt = span * span;
  let next: u32 = 0;
  fill({
    column(col) {
      // Columns skipped between the last and this one are empty: their
      // prefix entries all point at the current run count.
      for (; next <= col; next++) starts[next] = SC_Z0.len;
    },
    run(z0, z1, m) {
      SPAN_SINK.push(z0, z1, m.r, m.g, m.b, m.layer_idx, m.material_class, SUB_CELL_NONE, 0, 0, 0);
    },
  });
  for (; next <= column_cnt; next++) starts[next] = SC_Z0.len;
  return pack_scratch(span);
}

/** Packs the scratch spans into an exactly-sized run set. */
function pack_scratch(span: u32): ColumnRunSet {
  const starts = COLUMN_STARTS;
  const column_cnt = span * span;
  const set = alloc_column_run_set(SC_Z0.len, span);
  set.start.set(starts.subarray(0, column_cnt + 1));
  const n = SC_Z0.len;
  set.z0.set(SC_Z0.mem.subarray(0, n));
  set.z1.set(SC_Z1.mem.subarray(0, n));
  set.r.set(SC_R.mem.subarray(0, n));
  set.g.set(SC_G.mem.subarray(0, n));
  set.b.set(SC_B.mem.subarray(0, n));
  set.layer_taps.set(SC_LAYER.mem.subarray(0, n));
  set.material_class.set(SC_CLASS.mem.subarray(0, n));
  set.sub_sz.set(SC_SUB_SZ.mem.subarray(0, n));
  set.sub_sx.set(SC_SUB_SZ.mem.subarray(0, n));
  set.sub_sy.set(SC_SUB_SZ.mem.subarray(0, n));
  set.sub_ox.set(SC_SUB_OX.mem.subarray(0, n));
  set.sub_oy.set(SC_SUB_OY.mem.subarray(0, n));
  set.sub_oz.set(SC_SUB_OZ.mem.subarray(0, n));
  return set;
}

/**
 * Packs a column-per-entry array of runs into a run set.
 *
 * Not on the scan's own path — that writes into the spans directly — but a
 * test can build a chunk's runs by hand and get the same packed shape the
 * mesher consumes, without a ChunkVolume it does not care about.
 */
export function pack_column_runs(per_column: readonly (readonly MeshRun[])[], span: u32 = L2_BAND_CELL_SPAN): ColumnRunSet {
  let count: u32 = 0;
  for (const runs of per_column) count += runs.length;

  const set = alloc_column_run_set(count, span);
  const column_cnt = span * span;
  let w: u32 = 0;
  for (let c: u32 = 0; c < column_cnt; c++) {
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
  set.start[column_cnt] = w;
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
