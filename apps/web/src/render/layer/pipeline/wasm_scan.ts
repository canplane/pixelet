// @pxl/web/render/layer/pipeline/wasm_scan.ts

/**
 * The whole-chunk scan through the Rust scan (crates/pxl-core/src/scan.rs),
 * export by export, with the stepped scan's contract (voxel_scan.ts): one
 * open at a time, a row of columns a step, and a packed run set out of the
 * pool at the end — `starved` when the pool has no block for it. The harness
 * the Rust scan is held byte for byte to the TypeScript one through
 * (wasm_scan.test.ts); the chunks on screen are built by the module's own
 * build (core_build.ts). The composite is the module's own volume, opened by
 * its id.
 */

import type { Block } from "@xpute/core/ds/buddy.ts";

import { type PxlExports, STEP_DONE, STEP_NEED_MESH } from "@/wasm/pxl.ts";
import { carve_column_run_set, type ColumnRunSet } from "./column_run.ts";
import { type ChunkVolume, L2_BAND_CELL_SPAN } from "./volume.ts";
import type { MeshPool } from "./wasm_bake.ts";

export type WasmScanCursor = {
  done: boolean;
  starved: boolean;
  set: ColumnRunSet | null;
};

export class WasmScanner {
  private open: WasmScanCursor | null = null;
  constructor(private readonly pxl: PxlExports, private readonly pool: MeshPool) {}

  is_open(): boolean {
    return this.open !== null;
  }

  /** Opens a scan of `volume`, or null while one is open. The volume must
   * stay sealed until the scan closes. */
  begin(volume: ChunkVolume): WasmScanCursor | null {
    if (this.open !== null) return null;
    if (this.pxl.pxl_scan_begin_volume(volume.id) === 0) throw new Error("render: the wasm scan was open with no cursor naming it");
    this.open = { done: false, starved: false, set: null };
    return this.open;
  }

  /** One step: a row of columns, or the packing once every row is walked.
   * True once the set is packed. */
  step(cursor: WasmScanCursor): boolean {
    if (cursor !== this.open) throw new Error("render: step on a wasm scan that is not the open one");
    if (cursor.done) return true;
    const at = this.pxl.pxl_scan_step();
    if (at === STEP_NEED_MESH) {
      const bytes = this.pxl.pxl_scan_set_bytes();
      const block = this.pool.try_acquire(bytes);
      cursor.starved = block === null;
      if (block === null) return false;
      if (block.buf !== this.pxl.memory.buffer) throw new Error("render: a run set block outside the core memory");
      this.pxl.pxl_scan_set(block.byte_offset, bytes);
      cursor.set = carve_column_run_set(block, this.pxl.pxl_scan_finish(), L2_BAND_CELL_SPAN);
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
