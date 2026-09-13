// @pxl/web/render/layer/pipeline/voxel_scan.ts

/** Shared band scan used by rendering and raycasting. */

import type { i32, u32 } from "@xpute/core/kit/word.ts";
import { U_PER_CELL } from "@pxl/shared/bake/constant.ts";
import type { V3 } from "@pxl/shared/spec/space/v3.ts";

import { Lanes } from "@xpute/core/ds/lanes.ts";

import { CORE_ALLOC, CORE_POOL } from "./core_pool.ts";
import { carve_column_run_set, column_run_set_bytes, type ColumnRunSet } from "./column_run.ts";

import { band_voxel_idx, type ChunkVolume, get_chunk_band, L2_BAND_CELL_MASK, L2_BAND_CELL_SPAN, ordered_chunk_bands, sub_voxel_range, type SubVoxels, type VoxelBand } from "./volume.ts";
import { MaterialClass, SUB_CELL_NONE, unpack_sub_cell_fraction, type VoxelMaterial } from "./voxel.ts";

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

/** The prefix index, filled during the walk and copied into the block. */
const COLUMN_CNT: u32 = L2_BAND_CELL_SPAN * L2_BAND_CELL_SPAN;
const COLUMN_STARTS = CORE_ALLOC.take(Uint32Array, COLUMN_CNT + 1);

export type { ColumnRunSet };

/** A run set of `count` runs over `span`² columns, carved from the core pool. */
export function alloc_column_run_set(count: u32, span: u32 = L2_BAND_CELL_SPAN): ColumnRunSet {
  return carve_column_run_set(CORE_POOL.acquire(column_run_set_bytes(count, span)), count, span);
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
const SC = new Lanes({
  z0: Int32Array,
  z1: Int32Array,
  r: Uint8Array,
  g: Uint8Array,
  b: Uint8Array,
  layer: Uint32Array,
  cls: Uint8Array,
  sub_sz: Uint8Array,
  sub_ox: Uint8Array,
  sub_oy: Uint8Array,
  sub_oz: Uint8Array,
}, { init_cap: 1 << 13, alloc: CORE_ALLOC });

const SPAN_SINK: RunSink = {
  push(z0, z1, r, g, b, layer_idx, material_class, sub_sz, sub_ox, sub_oy, sub_oz) {
    const i = SC.push();
    const m = SC.mem;
    m.z0[i] = z0;
    m.z1[i] = z1;
    m.r[i] = r;
    m.g[i] = g;
    m.b[i] = b;
    m.layer[i] = layer_idx * 0x01010101;
    m.cls[i] = material_class;
    m.sub_sz[i] = sub_sz;
    m.sub_ox[i] = sub_ox;
    m.sub_oy[i] = sub_oy;
    m.sub_oz[i] = sub_oz;
    return i;
  },
  extend(index, z1) {
    SC.mem.z1[index] = z1;
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
  /** Next row to walk, 0..L2_BAND_CELL_SPAN. */
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

/** Opens a scan of `volume`, or returns null while another is open. */
export function begin_column_scan(volume: ChunkVolume): ColumnScan | null {
  if (open_scan !== null) return null;
  SC.clear();
  open_scan = { volume, next_row: 0 };
  return open_scan;
}

/** Whether every row has been walked. */
export function scan_complete(scan: ColumnScan): boolean {
  return scan.next_row >= L2_BAND_CELL_SPAN;
}

/**
 * Walks up to `rows` more rows. Returns true once the whole chunk is walked,
 * at which point `finish_column_scan` packs the result.
 */
export function scan_column_rows(scan: ColumnScan, rows: u32 = SCAN_ROWS_PER_STEP): boolean {
  const starts = COLUMN_STARTS;
  const span = L2_BAND_CELL_SPAN;
  const end = Math.min(span, scan.next_row + rows);
  for (let cy: u32 = scan.next_row; cy < end; cy++) {
    const row_base = cy * span;
    for (let cx: u32 = 0; cx < span; cx++) {
      starts[row_base + cx] = SC.len;
      walk_column_runs(scan.volume, cx, cy, undefined, SPAN_SINK);
    }
  }
  scan.next_row = end;
  return scan_complete(scan);
}

/** Packs a finished scan into an exactly-sized set and closes it. */
export function finish_column_scan(scan: ColumnScan): ColumnRunSet {
  if (scan !== open_scan || !scan_complete(scan)) {
    throw new Error("render: finish_column_scan on a scan that is not the open, complete one");
  }
  COLUMN_STARTS[COLUMN_CNT] = SC.len;
  // A baked sub-voxel object is a cube, so its three extents are one value;
  // only the coarse mesher writes them apart. pack_scratch copies sub_sz into
  // all three.
  const set = pack_scratch(L2_BAND_CELL_SPAN);
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
  SC.clear();
  const starts = COLUMN_STARTS;
  const column_cnt = span * span;
  let next: u32 = 0;
  fill({
    column(col) {
      // Columns skipped between the last and this one are empty: their
      // prefix entries all point at the current run count.
      for (; next <= col; next++) starts[next] = SC.len;
    },
    run(z0, z1, m) {
      SPAN_SINK.push(z0, z1, m.r, m.g, m.b, m.layer_idx, m.material_class, SUB_CELL_NONE, 0, 0, 0);
    },
  });
  for (; next <= column_cnt; next++) starts[next] = SC.len;
  return pack_scratch(span);
}

/** Packs the scratch spans into an exactly-sized run set. */
function pack_scratch(span: u32): ColumnRunSet {
  const starts = COLUMN_STARTS;
  const column_cnt = span * span;
  const set = alloc_column_run_set(SC.len, span);
  set.start.set(starts.subarray(0, column_cnt + 1));
  const n = SC.len;
  const m = SC.mem;
  set.z0.set(m.z0.subarray(0, n));
  set.z1.set(m.z1.subarray(0, n));
  set.r.set(m.r.subarray(0, n));
  set.g.set(m.g.subarray(0, n));
  set.b.set(m.b.subarray(0, n));
  set.layer_taps.set(m.layer.subarray(0, n));
  set.material_class.set(m.cls.subarray(0, n));
  set.sub_sz.set(m.sub_sz.subarray(0, n));
  set.sub_sx.set(m.sub_sz.subarray(0, n));
  set.sub_sy.set(m.sub_sz.subarray(0, n));
  set.sub_ox.set(m.sub_ox.subarray(0, n));
  set.sub_oy.set(m.sub_oy.subarray(0, n));
  set.sub_oz.set(m.sub_oz.subarray(0, n));
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
