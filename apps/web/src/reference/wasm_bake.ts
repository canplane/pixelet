// @pxl/web/reference/wasm_bake.ts

/**
 * The chunk bake through the Rust mesher (crates/pxl/core/src/bake/mesher.rs),
 * export by export, with the stepped bake's contract: one open at a time, a
 * step per call, `starved` when the pool has no block for the mesh, and the
 * counters added to the context's. What the chunks on screen are built
 * with is the module's own build (core_build.ts); this is the harness the
 * Rust mesher is held byte for byte to the TypeScript one through
 * (wasm_bake.test.ts). Every block is in the core memory.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import type { Block } from "@xpute/core/ds/buddy.ts";

import type { LodStep } from "@/module/world/lod.ts";
import { type PxlExports, STEP_DONE, STEP_NEED_MESH } from "@/wasm/pxl.ts";
import type { ColumnRunSet } from "@/reference/column_run.ts";
import type { BakeContext } from "@/reference/chunk_mesher.ts";
import { baked_mesh_bytes, type BakedChunkMesh, carve_baked_mesh } from "@/render/mesh/mesh_block.ts";

/** What one bake added to its context's counters. */
type BakeCounters = {
  total_runs: u32;
  culled: u32;
  faces_possible: u32;
  faces_kept: u32;
  coarse_total: u32;
  coarse_low_fill: u32;
};

function add_bake_counters(ctx: BakeContext, counters: BakeCounters): void {
  ctx.occlusion.total_runs += counters.total_runs;
  ctx.occlusion.culled += counters.culled;
  ctx.occlusion.faces_possible += counters.faces_possible;
  ctx.occlusion.faces_kept += counters.faces_kept;
  ctx.coarse_instances.total += counters.coarse_total;
  ctx.coarse_instances.low_fill += counters.coarse_low_fill;
}

/** Where the meshes go: the mesh section's pool, or in a test one that
 * refuses. Its blocks must lie in the memory the module was instantiated
 * over. */
export type MeshPool = {
  try_acquire(bytes: u32): Block | null;
  release(block: Block): void;
};

export type WasmBakeCursor = {
  done: boolean;
  starved: boolean;
  /** The block the mesh is being written into, once the bake asked for one. */
  block: Block | null;
  mesh: BakedChunkMesh | null;
};

const RESULT_WORDS: u32 = 9;

export class WasmBaker {
  private open: WasmBakeCursor | null = null;
  /** The module's cumulative counters as of the last finished bake. */
  private seen: Uint32Array = new Uint32Array(6);
  /** The module heap's use after the last finished bake. */
  heap_used: u32 = 0;

  /** `ctx` is where the counters go and `pool` where the meshes do — the
   * same two the stepped path bakes with. */
  constructor(readonly pxl: PxlExports, private readonly pool: MeshPool, private readonly ctx: BakeContext) {}

  is_open(): boolean {
    return this.open !== null;
  }

  begin(set: ColumnRunSet, lod_step: LodStep): WasmBakeCursor | null {
    if (this.open !== null) return null;
    if (set.block.buf !== this.pxl.memory.buffer) throw new Error("render: a wasm bake of a run set outside the core memory");
    if (this.pxl.pxl_bake_begin(set.block.byte_offset, set.count, set.span, lod_step) === 0) {
      throw new Error("render: the wasm bake was open with no cursor naming it");
    }
    this.open = { done: false, starved: false, block: null, mesh: null };
    return this.open;
  }

  /** A block for the mesh, or null when the pool refused (`starved`). */
  private mesh_block(cursor: WasmBakeCursor, words: u32): Block | null {
    const block = this.pool.try_acquire(words * 4);
    cursor.starved = block === null;
    if (block !== null && block.buf !== this.pxl.memory.buffer) throw new Error("render: a mesh block outside the core memory");
    return block;
  }

  step(cursor: WasmBakeCursor): boolean {
    if (cursor !== this.open) throw new Error("render: step on a wasm bake that is not the open one");
    if (cursor.done) return true;
    const at = this.pxl.pxl_bake_step();
    if (at === STEP_NEED_MESH) {
      const words = this.pxl.pxl_bake_mesh_words();
      const block = this.mesh_block(cursor, words);
      if (block === null) return false;
      cursor.block = block;
      this.pxl.pxl_bake_mesh(block.byte_offset, words);
      return false;
    }
    cursor.starved = false;
    if (at !== STEP_DONE) return false;
    const result = new Uint32Array(this.pxl.memory.buffer, this.pxl.pxl_bake_finish(), RESULT_WORDS);
    const vertex_cnt = result[0], index_cnt = result[1];
    // A mesh of nothing was never asked for; it gets the empty block the
    // TypeScript bake would have carved, wherever that is.
    const block = cursor.block ?? this.pool.try_acquire(baked_mesh_bytes(vertex_cnt, index_cnt));
    if (block === null) {
      cursor.starved = true;
      return false;
    }
    cursor.block = block;
    cursor.mesh = carve_baked_mesh(block, vertex_cnt, index_cnt);
    add_bake_counters(this.ctx, this.counters_since(result.subarray(2, 8)));
    this.heap_used = result[8];
    cursor.done = true;
    return true;
  }

  finish(cursor: WasmBakeCursor): BakedChunkMesh {
    if (cursor !== this.open || !cursor.done) throw new Error("render: finish on a wasm bake that is not the open, complete one");
    this.open = null;
    return cursor.mesh!;
  }

  abort(cursor: WasmBakeCursor): void {
    if (cursor !== this.open) return;
    if (!cursor.done) this.pxl.pxl_bake_abort();
    if (cursor.block) this.pool.release(cursor.block);
    this.open = null;
  }

  private counters_since(now: Uint32Array): BakeCounters {
    const d = (i: number) => (now[i] - this.seen[i]) >>> 0;
    const counters = { total_runs: d(0), culled: d(1), faces_possible: d(2), faces_kept: d(3), coarse_total: d(4), coarse_low_fill: d(5) };
    this.seen.set(now);
    return counters;
  }
}
