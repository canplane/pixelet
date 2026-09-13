// @pxl/web/render/layer/pipeline/wasm_bake.ts

/**
 * The chunk bake through the Rust mesher (crates/pxl-core/src/mesher.rs),
 * with the stepped bake's contract: one open at a time, a step per call
 * under the caller's budget, `starved` when the mesh section has no block
 * for the mesh, and the counters the ledger reads added to the context's.
 *
 * The module shares the core memory, so nothing is copied: the run set is
 * baked in the block it lives in, and the mesh is written into the block
 * acquired for it here — at the step the bake first knows its size, which
 * is where the TypeScript bake acquires its own, and where a refusal leaves
 * the bake waiting for the next tick.
 *
 * A block the module cannot reach — the mesh section past its range spills
 * to plain buffers (core_pool.ts), and a run set or a mesh can land in one
 * — is the module's limit, not an error. A mesh small enough is staged in
 * the module's own heap and copied out into the block when the bake is
 * done, which on a phone at a wide zoom is most of them: the section is
 * over there as a rule, and a bake restarted the TypeScript way would give
 * the phone's gain back. Too large to stage, the bake reports `spilled`
 * and its caller bakes that chunk the TypeScript way (core_bake.ts). The
 * section overflowing is the byte budget's problem (BACKLOG P1), not this
 * path's to solve by refusing.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import type { Block } from "@xpute/core/ds/buddy.ts";

import type { LodStep } from "@/render/lod.ts";
import { type PxlExports, STEP_DONE, STEP_NEED_MESH } from "@/wasm/pxl.ts";
import type { ColumnRunSet } from "./column_run.ts";
import { type BakeContext, baked_mesh_bytes, type BakedChunkMesh, carve_baked_mesh } from "./chunk_mesher.ts";
import { add_bake_counters, type BakeCounters } from "./bake_command.ts";

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
  /** The mesh's block was outside the core memory: this bake cannot go
   * on in the module. Abort it and bake the chunk the TypeScript way. */
  spilled: boolean;
  /** The block the mesh is being written into, once the bake asked for one. */
  block: Block | null;
  /** Where the mesh is staged in the module's heap, when the block was out
   * of the module's reach; copied out at the end. */
  staged: u32;
  mesh: BakedChunkMesh | null;
};

const RESULT_WORDS: u32 = 9;

export class WasmBaker {
  private open: WasmBakeCursor | null = null;
  /** The module's cumulative counters as of the last finished bake. */
  private seen: Uint32Array = new Uint32Array(6);
  /** The module heap's use after the last finished bake, for the ledger. */
  heap_used: u32 = 0;
  /** Meshes staged in the module's heap and copied out, for the ledger. */
  staged = 0;

  /** `ctx` is where the counters go and `pool` where the meshes do — the
   * same two the stepped path bakes with. */
  constructor(readonly pxl: PxlExports, private readonly pool: MeshPool, private readonly ctx: BakeContext) {}

  is_open(): boolean {
    return this.open !== null;
  }

  /** Whether the module can read `set` where it is. A caller bakes a set
   * that is not the TypeScript way. */
  reaches(set: ColumnRunSet): boolean {
    return set.block.buf === this.pxl.memory.buffer;
  }

  begin(set: ColumnRunSet, lod_step: LodStep): WasmBakeCursor | null {
    if (this.open !== null) return null;
    if (!this.reaches(set)) throw new Error("render: a wasm bake of a run set outside the core memory");
    if (this.pxl.pxl_bake_begin(set.block.byte_offset, set.count, set.span, lod_step) === 0) {
      throw new Error("render: the wasm bake was open with no cursor naming it");
    }
    this.open = { done: false, starved: false, spilled: false, block: null, staged: 0, mesh: null };
    return this.open;
  }

  /** A block for the mesh, or null when the pool refused (`starved`). A
   * block the module cannot reach is kept and the mesh staged for it, or,
   * too large to stage, released with the cursor `spilled`. */
  private mesh_block(cursor: WasmBakeCursor, words: u32): Block | null {
    const block = this.pool.try_acquire(words * 4);
    cursor.starved = block === null;
    if (block === null) return null;
    if (block.buf !== this.pxl.memory.buffer) {
      const staged = this.pxl.pxl_bake_mesh_staged(words);
      if (staged === 0) {
        this.pool.release(block);
        cursor.spilled = true;
        return null;
      }
      cursor.staged = staged;
      this.staged++;
    }
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
      if (cursor.staged === 0) this.pxl.pxl_bake_mesh(block.byte_offset, words);
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
    if (cursor.staged !== 0) {
      const bytes = baked_mesh_bytes(vertex_cnt, index_cnt);
      new Uint8Array(block.buf, block.byte_offset, bytes).set(new Uint8Array(this.pxl.memory.buffer, cursor.staged, bytes));
    }
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
