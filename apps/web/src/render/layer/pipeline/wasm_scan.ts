// @pxl/web/render/layer/pipeline/wasm_scan.ts

/**
 * The whole-chunk scan through the Rust scan (crates/pxl-core/src/scan.rs),
 * with the stepped scan's contract (voxel_scan.ts): one open at a time, a
 * row of columns a step under the caller's budget, and a packed run set
 * out of the mesh section at the end — `starved` when the section has no
 * block for it, and the scan waits.
 *
 * The composite's bands are core pool blocks and its overflow sub-voxels
 * are lanes in the core memory, so the module reads them where they are.
 * What crosses is a table naming them — one row of addresses a band — in
 * a lane of the fixed section written here and read there.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import type { Block } from "@xpute/core/ds/buddy.ts";

import { type PxlExports, STEP_DONE, STEP_NEED_MESH } from "@/wasm/pxl.ts";
import { FIXED_ALLOC } from "./core_pool.ts";
import { carve_column_run_set, type ColumnRunSet } from "./column_run.ts";
import { type ChunkVolume, L2_BAND_CELL_SPAN } from "./volume.ts";
import type { MeshPool } from "./wasm_bake.ts";

/** Words a band's row takes: crates/pxl-wasm/src/lib.rs's BAND_WORDS. */
const BAND_WORDS: u32 = 10;
/** Rows the table has: bands a chunk can hold (signed Z, 64 cells each)
 * that any terrain reaches, with room over. */
const MAX_BANDS: u32 = 256;

export type WasmScanCursor = {
  done: boolean;
  starved: boolean;
  set: ColumnRunSet | null;
};

export class WasmScanner {
  private open: WasmScanCursor | null = null;
  private readonly table: Uint32Array;

  constructor(private readonly pxl: PxlExports, private readonly pool: MeshPool) {
    this.table = FIXED_ALLOC.take(Uint32Array, MAX_BANDS * BAND_WORDS);
    if (this.table.buffer !== pxl.memory.buffer) throw new Error("render: the scan's band table is outside the core memory");
  }

  is_open(): boolean {
    return this.open !== null;
  }

  /** Opens a scan of `volume`, or null while one is open. The volume's
   * bands and lanes must stay where they are until the scan closes. */
  begin(volume: ChunkVolume): WasmScanCursor | null {
    if (this.open !== null) return null;
    const bands = volume.bands;
    if (bands.length > MAX_BANDS) throw new Error(`render: a volume of ${bands.length} bands, past the scan table's ${MAX_BANDS}`);
    const memory = this.pxl.memory.buffer;
    const t = this.table;
    for (let i = 0; i < bands.length; i++) {
      const band = bands[i];
      const e = band.extra;
      for (const lane of [band.material, band.layer_idx, band.sub_cell, e.col.mem, e.z.mem, e.material.mem, e.layer_idx.mem, e.sub.mem]) {
        if (lane && lane.buffer !== memory) throw new Error("render: a wasm scan of a band outside the core memory");
      }
      const row = i * BAND_WORDS;
      t[row] = band.band_z >>> 0;
      t[row + 1] = band.material.byteOffset;
      t[row + 2] = band.layer_idx.byteOffset;
      t[row + 3] = band.sub_cell ? band.sub_cell.byteOffset : 0;
      t[row + 4] = e.col.len;
      t[row + 5] = e.col.mem.byteOffset;
      t[row + 6] = e.z.mem.byteOffset;
      t[row + 7] = e.material.mem.byteOffset;
      t[row + 8] = e.layer_idx.mem.byteOffset;
      t[row + 9] = e.sub.mem.byteOffset;
    }
    if (this.pxl.pxl_scan_begin(t.byteOffset, bands.length) === 0) throw new Error("render: the wasm scan was open with no cursor naming it");
    this.open = { done: false, starved: false, set: null };
    return this.open;
  }

  /** One step: a row of columns, or the packing once every row is walked.
   * True once the set is packed. */
  step(cursor: WasmScanCursor): boolean {
    if (cursor.done) return true;
    const at = this.pxl.pxl_scan_step();
    if (at === STEP_NEED_MESH) {
      const bytes = this.pxl.pxl_scan_set_bytes();
      const block = this.pool.try_acquire(bytes);
      cursor.starved = block === null;
      if (block === null) return false;
      if (block.buf !== this.pxl.memory.buffer) throw new Error("render: a run set block outside the core memory");
      this.pxl.pxl_scan_set(block.byte_offset, bytes);
      const count = this.pxl.pxl_scan_finish();
      cursor.set = carve_column_run_set(block, count, L2_BAND_CELL_SPAN);
      cursor.done = true;
      return true;
    }
    cursor.starved = false;
    return at === STEP_DONE;
  }

  finish(cursor: WasmScanCursor): ColumnRunSet {
    if (cursor !== this.open || !cursor.done) throw new Error("render: finish on a wasm scan that is not the open, complete one");
    this.open = null;
    return cursor.set!;
  }

  abort(cursor: WasmScanCursor): void {
    if (cursor !== this.open) return;
    if (!cursor.done) this.pxl.pxl_scan_abort();
    if (cursor.set) this.pool.release(cursor.set.block as Block);
    this.open = null;
  }
}
