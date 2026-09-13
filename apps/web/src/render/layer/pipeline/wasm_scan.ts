// @pxl/web/render/layer/pipeline/wasm_scan.ts

/**
 * The whole-chunk scan through the Rust scan (crates/pxl-core/src/scan.rs),
 * with the stepped scan's contract (voxel_scan.ts): one open at a time, a
 * row of columns a step under the caller's budget, and a packed run set
 * out of the mesh section at the end — `starved` when the section has no
 * block for it, and the scan waits.
 *
 * The composite is the module's own volume (volume.ts), so the scan is
 * opened by its id and reads the bands where the module painted them.
 */

import type { Block } from "@xpute/core/ds/buddy.ts";

import { type PxlExports, STEP_DONE, STEP_NEED_MESH } from "@/wasm/pxl.ts";
import { carve_column_run_set, type ColumnRunSet } from "./column_run.ts";
import { type ChunkVolume, L2_BAND_CELL_SPAN } from "./volume.ts";
import type { MeshPool } from "./wasm_bake.ts";

export type WasmScanCursor = {
  done: boolean;
  starved: boolean;
  /** The set's block was outside the core memory: this scan cannot go on
   * in the module. Abort it and scan the volume the TypeScript way. */
  spilled: boolean;
  set: ColumnRunSet | null;
};

export class WasmScanner {
  private open: WasmScanCursor | null = null;
  /** Sets staged in the module's heap and copied out, for the ledger. */
  staged = 0;

  constructor(private readonly pxl: PxlExports, private readonly pool: MeshPool) {}

  is_open(): boolean {
    return this.open !== null;
  }

  /** Whether the module can read `volume` where it is: every volume is the
   * module's own now, so always. Kept as the question core_bake asks. */
  reaches(_volume: ChunkVolume): boolean {
    return true;
  }

  /** Opens a scan of `volume`, or null while one is open. The volume must
   * stay sealed until the scan closes. */
  begin(volume: ChunkVolume): WasmScanCursor | null {
    if (this.open !== null) return null;
    if (this.pxl.pxl_scan_begin_volume(volume.id) === 0) throw new Error("render: the wasm scan was open with no cursor naming it");
    this.open = { done: false, starved: false, spilled: false, set: null };
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
      if (block.buf !== this.pxl.memory.buffer) {
        // Out of the module's reach: staged in its heap and copied out, or
        // too large for that and scanned the TypeScript way (wasm_bake.ts).
        const staged = this.pxl.pxl_scan_set_staged(bytes);
        if (staged === 0) {
          this.pool.release(block);
          cursor.spilled = true;
          return false;
        }
        new Uint8Array(block.buf, block.byte_offset, bytes).set(new Uint8Array(this.pxl.memory.buffer, staged, bytes));
        this.staged++;
      } else {
        this.pxl.pxl_scan_set(block.byte_offset, bytes);
      }
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
