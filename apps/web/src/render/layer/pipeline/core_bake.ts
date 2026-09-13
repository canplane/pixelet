// @pxl/web/render/layer/pipeline/core_bake.ts

/**
 * The main thread's bake context: the mesher's scratch and counters over the
 * core pool, which is where every mesh the renderer draws lives. Kept out of
 * chunk_mesher.ts so that importing the mesher reserves nothing — a worker
 * builds its own context over its own memory, and only the main thread
 * imports this one.
 */

import type { LodStep } from "@/render/lod.ts";
import { CORE_MEMORY, CORE_POOL, FIXED_ALLOC } from "./core_pool.ts";
import { abort_bake, BakeContext, type BakedChunkMesh, begin_bake, finish_bake, step_bake } from "./chunk_mesher.ts";
import type { ColumnRunSet } from "./column_run.ts";
import { lazy_wasm_baker } from "./wasm_bake.ts";

export const CORE_BAKE: BakeContext = new BakeContext(CORE_POOL, FIXED_ALLOC);

/** Hands one baked mesh's block back to the core pool. Every view into it is
 * dead afterwards. */
export function release_chunk_mesh(mesh: BakedChunkMesh): void {
  CORE_POOL.release(mesh.block);
}

/** Which mesher a bake on this thread runs: the TypeScript one, or the Rust
 * one through the pxl module. */
export type CoreMesher = "ts" | "wasm";

/**
 * A bake stepped on this thread under the frame budget. Both meshers keep
 * one contract — one open at a time, `starved` when the mesh section has no
 * block, the counters into CORE_BAKE — so a chunk drives either through this.
 */
export type CoreBake = {
  readonly mesher: CoreMesher;
  /** One step; true once the mesh is in its block. */
  advance(): boolean;
  starved(): boolean;
  finish(): BakedChunkMesh;
  abort(): void;
};

const core_wasm_baker = lazy_wasm_baker(CORE_MEMORY, CORE_POOL, CORE_BAKE);

/** The module heap's use after its last bake, for the ledger; null until
 * the module has baked. */
export function core_wasm_heap_used(): number | null {
  const baker = core_wasm_baker();
  return typeof baker === "string" ? null : baker.heap_used;
}

/** The mesher a bake asking for `wanted` gets: the Rust one unless its
 * module failed to load or there is no core memory for it to share, when
 * the TypeScript one stands in — the mesh is the same either way. Asking
 * also starts the module's load. */
export function resolve_core_mesher(wanted: CoreMesher): CoreMesher {
  return wanted === "wasm" && core_wasm_baker() === "failed" ? "ts" : wanted;
}

/**
 * Opens a bake of `set` at `step` on `mesher`, or null when that mesher's
 * one bake is taken — or, for the Rust mesher, while its module is still
 * arriving or has failed (resolve_core_mesher first). Either way the caller
 * is blocked and asks again next tick.
 */
export function begin_core_bake(mesher: CoreMesher, set: ColumnRunSet, step: LodStep): CoreBake | null {
  if (mesher === "ts") {
    const cursor = begin_bake(CORE_BAKE, set, step);
    if (cursor === null) return null;
    return { mesher, advance: () => step_bake(cursor), starved: () => cursor.starved, finish: () => finish_bake(cursor), abort: () => abort_bake(cursor) };
  }
  const baker = core_wasm_baker();
  if (typeof baker === "string") return null;
  const cursor = baker.begin(set, step);
  if (cursor === null) return null;
  return { mesher, advance: () => baker.step(cursor), starved: () => cursor.starved, finish: () => baker.finish(cursor), abort: () => baker.abort(cursor) };
}
