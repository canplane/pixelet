// @pxl/web/render/layer/pipeline/wasm_bake.ts

/**
 * The chunk bake through the Rust mesher (crates/pxl-core/src/mesher.rs),
 * with the stepped bake's contract: one open at a time, a step per call
 * under the caller's budget, `starved` when the pool has no block for the
 * mesh, and the counters the ledger reads added to the context's.
 *
 * The module keeps its own memory for now, so a bake is a copy in — the
 * run set's bytes — and a copy out — the mesh's words, into a block from
 * the pool. The copy out is the last step: the Rust bake completes into its
 * own memory and only then asks the pool, so a refusal leaves the finished
 * mesh where it is and the next step asks again.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import type { BuddyAllocator } from "@xpute/core/ds/buddy.ts";

import type { LodStep } from "@/render/lod.ts";
import { instantiate_pxl, type PxlExports } from "@/wasm/pxl.ts";
import { column_run_set_bytes, type ColumnRunSet } from "./column_run.ts";
import { type BakeContext, baked_mesh_bytes, type BakedChunkMesh, carve_baked_mesh } from "./chunk_mesher.ts";
import { add_bake_counters, type BakeCounters } from "./bake_command.ts";

export type WasmBakeCursor = {
  done: boolean;
  starved: boolean;
  /** The Rust bake has completed; what remains is the copy out. */
  baked: boolean;
  mesh: BakedChunkMesh | null;
};

const RESULT_WORDS: u32 = 9;

export class WasmBaker {
  private open: WasmBakeCursor | null = null;
  /** The module's cumulative counters as of the last finished bake. */
  private seen: Uint32Array = new Uint32Array(6);
  /** Where the completed bake's result words are, once pxl_bake_finish has
   * closed it and until its mesh is copied out; 0 before. */
  private result: u32 = 0;

  /** `ctx` is where the counters go and `pool` where the meshes do — the
   * same two the stepped path bakes with. */
  constructor(private readonly pxl: PxlExports, private readonly pool: BuddyAllocator, private readonly ctx: BakeContext) {}

  is_open(): boolean {
    return this.open !== null;
  }

  begin(set: ColumnRunSet, lod_step: LodStep): WasmBakeCursor | null {
    if (this.open !== null) return null;
    const bytes = column_run_set_bytes(set.count, set.span);
    const at = this.pxl.pxl_bake_input(set.count, set.span);
    new Uint8Array(this.pxl.memory.buffer, at, bytes).set(new Uint8Array(set.block.buf, set.block.byte_offset, bytes));
    if (this.pxl.pxl_bake_begin(lod_step) === 0) throw new Error("render: the wasm bake was open with no cursor naming it");
    this.open = { done: false, starved: false, baked: false, mesh: null };
    return this.open;
  }

  step(cursor: WasmBakeCursor): boolean {
    if (cursor.done) return true;
    if (!cursor.baked) {
      cursor.baked = this.pxl.pxl_bake_step() !== 0;
      if (!cursor.baked) return false;
    }
    cursor.done = this.copy_out(cursor);
    cursor.starved = !cursor.done;
    return cursor.done;
  }

  finish(cursor: WasmBakeCursor): BakedChunkMesh {
    if (cursor !== this.open || !cursor.done) throw new Error("render: finish on a wasm bake that is not the open, complete one");
    this.open = null;
    return cursor.mesh!;
  }

  abort(cursor: WasmBakeCursor): void {
    if (cursor !== this.open) return;
    // A completed bake is closed on the Rust side already, by the copy out.
    if (!cursor.baked) this.pxl.pxl_bake_abort();
    this.result = 0;
    if (cursor.mesh) this.pool.release(cursor.mesh.block);
    this.open = null;
  }

  private copy_out(cursor: WasmBakeCursor): boolean {
    if (this.result === 0) this.result = this.pxl.pxl_bake_finish();
    const result = new Uint32Array(this.pxl.memory.buffer, this.result, RESULT_WORDS);
    const vertex_cnt = result[0], index_cnt = result[1];
    const bytes = baked_mesh_bytes(vertex_cnt, index_cnt);
    const block = this.pool.try_acquire(bytes);
    if (block === null) return false;
    new Uint8Array(block.buf, block.byte_offset, bytes).set(new Uint8Array(this.pxl.memory.buffer, result[2], bytes));
    cursor.mesh = carve_baked_mesh(block, vertex_cnt, index_cnt);
    add_bake_counters(this.ctx, this.counters_since(result.subarray(3)));
    this.result = 0;
    return true;
  }

  private counters_since(now: Uint32Array): BakeCounters {
    const d = (i: number) => (now[i] - this.seen[i]) >>> 0;
    const counters = { total_runs: d(0), culled: d(1), faces_possible: d(2), faces_kept: d(3), coarse_total: d(4), coarse_low_fill: d(5) };
    this.seen.set(now);
    return counters;
  }
}

/**
 * Instantiates the module once, on first use: "loading" until it has
 * arrived, so a caller treats the bake as blocked and asks again next tick,
 * and "failed" for good if it does not arrive, so a caller can bake another
 * way rather than wait forever.
 */
export function lazy_wasm_baker(pool: BuddyAllocator, ctx: BakeContext): () => WasmBaker | "loading" | "failed" {
  let state: WasmBaker | "idle" | "loading" | "failed" = "idle";
  return () => {
    if (state !== "idle") return state;
    state = "loading";
    instantiate_pxl().then(
      (pxl) => {
        state = new WasmBaker(pxl, pool, ctx);
      },
      (err) => {
        state = "failed";
        console.error("[render/wasm_bake] the pxl module did not load; bakes fall back to the TypeScript mesher:", err);
      },
    );
    return state;
  };
}
