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
import type { ChunkVolume } from "./volume.ts";
import { abort_column_scan, begin_column_scan, column_scan_open, finish_column_scan, scan_column_rows } from "./voxel_scan.ts";
import { lazy_wasm_baker } from "./wasm_bake.ts";
import { WasmScanner } from "./wasm_scan.ts";

export const CORE_BAKE: BakeContext = new BakeContext(CORE_POOL, FIXED_ALLOC);

/** Hands one baked mesh's block back to the core pool. Every view into it is
 * dead afterwards. */
export function release_chunk_mesh(mesh: BakedChunkMesh): void {
  CORE_POOL.release(mesh.block);
}

/** Which mesher a bake on this thread runs — and which scan, the two go
 * together: the TypeScript ones, or the Rust ones through the pxl module. */
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

const load_wasm_baker = lazy_wasm_baker(CORE_MEMORY, CORE_POOL, CORE_BAKE);

/**
 * A module that trapped is not used again: its statics are whatever the
 * trap left, so every later export would fail too. The trap's error is
 * reported once and the TypeScript path takes over for the session.
 */
let wasm_trapped = false;

/** How often a module job went the TypeScript way, and why, for the
 * ledger: `spilled` is the mesh section past its range — the byte budget's
 * signal (BACKLOG P1) — `unreached` a set or volume already outside it. */
export const core_fallback_stats = { spilled: 0, unreached: 0, trapped: 0 };
function core_wasm_baker(): ReturnType<typeof load_wasm_baker> {
  return wasm_trapped ? "failed" : load_wasm_baker();
}

function trapped(err: unknown): void {
  if (wasm_trapped) return;
  wasm_trapped = true;
  console.error("[render/core_bake] the pxl module trapped; scans and bakes fall back to the TypeScript path for the rest of the session:", err);
}

/** The one Rust scanner, over the same module as the baker; made when the
 * module has arrived. */
let core_wasm_scanner: WasmScanner | null = null;
function wasm_scanner(): WasmScanner | null {
  const baker = core_wasm_baker();
  if (typeof baker === "string") return null;
  return core_wasm_scanner ??= new WasmScanner(baker.pxl, CORE_POOL);
}

/** The module heap's use after its last bake, for the ledger; null until
 * the module has baked. */
export function core_wasm_heap_used(): number | null {
  const baker = core_wasm_baker();
  return typeof baker === "string" ? null : baker.heap_used;
}

/** Meshes and sets the module staged in its heap and copied out because
 * their block was past the mesh section's range, for the ledger. */
export function core_wasm_staged(): { meshes: number; sets: number } {
  const baker = core_wasm_baker();
  return { meshes: typeof baker === "string" ? 0 : baker.staged, sets: core_wasm_scanner?.staged ?? 0 };
}

/** The mesher a bake asking for `wanted` gets: the Rust one unless its
 * module failed to load or there is no core memory for it to share, when
 * the TypeScript one stands in — the mesh is the same either way. Asking
 * also starts the module's load. */
export function resolve_core_mesher(wanted: CoreMesher): CoreMesher {
  return wanted === "wasm" && core_wasm_baker() === "failed" ? "ts" : wanted;
}

/**
 * A whole-chunk scan stepped on this thread, with the bake's contract: one
 * open at a time (`core_scan_open` says, so a would-be holder can skip
 * composing), `starved` when the mesh section has no block for the set.
 */
export type CoreScan = {
  readonly mesher: CoreMesher;
  advance(): boolean;
  starved(): boolean;
  finish(): ColumnRunSet;
  abort(): void;
};

export function core_scan_open(mesher: CoreMesher): boolean {
  if (mesher === "ts") return column_scan_open();
  return wasm_scanner()?.is_open() ?? false;
}

/**
 * A job that runs in the module until the module cannot reach a block it
 * needs — a set or a mesh the pool spilled past the section — and from
 * there goes the TypeScript way, restarted from the beginning: the same
 * result, a step later. `open_ts` may refuse while the TypeScript slot is
 * taken; the job then reads as starved and asks again on the next advance.
 * Reports the mesher it was opened for, so a caller comparing it to the
 * setting sees no change.
 */
type Job<T> = { advance(): boolean; starved(): boolean; finish(): T; abort(): void };

function with_fallback<T>(wasm: Job<T> & { spilled(): boolean }, open_ts: () => Job<T> | null): Job<T> & { readonly mesher: CoreMesher } {
  let inner: Job<T> | null = wasm;
  let waiting = false;
  const fall_back = () => {
    inner = open_ts();
    waiting = inner === null;
  };
  return {
    mesher: "wasm",
    advance() {
      if (inner === null) {
        fall_back();
        if (inner === null) return false;
      }
      let done: boolean;
      try {
        done = inner.advance();
      } catch (err) {
        // A trap in the module (an exhausted heap, a bug): this job and every
        // one after it go the TypeScript way. The module is left as it is —
        // nothing in it can be trusted to abort cleanly.
        if (inner !== wasm) throw err;
        core_fallback_stats.trapped++;
        trapped(err);
        fall_back();
        return false;
      }
      if (inner === wasm && wasm.spilled()) {
        core_fallback_stats.spilled++;
        wasm.abort();
        fall_back();
        return false;
      }
      return done;
    },
    starved: () => waiting || (inner?.starved() ?? false),
    finish: () => inner!.finish(),
    abort: () => {
      if (inner === wasm && wasm_trapped) return;
      inner?.abort();
    },
  };
}

function ts_scan(volume: ChunkVolume): Job<ColumnRunSet> | null {
  const cursor = begin_column_scan(volume);
  if (cursor === null) return null;
  return { advance: () => scan_column_rows(cursor), starved: () => false, finish: () => finish_column_scan(cursor), abort: () => abort_column_scan(cursor) };
}

function ts_bake(set: ColumnRunSet, step: LodStep): Job<BakedChunkMesh> | null {
  const cursor = begin_bake(CORE_BAKE, set, step);
  if (cursor === null) return null;
  return { advance: () => step_bake(cursor), starved: () => cursor.starved, finish: () => finish_bake(cursor), abort: () => abort_bake(cursor) };
}

/**
 * Opens a scan of `volume` on `mesher`, or null while its one scan is
 * taken or, for the Rust one, while the module is still arriving. A volume
 * the module cannot reach is scanned the TypeScript way whatever `mesher`.
 */
export function begin_core_scan(mesher: CoreMesher, volume: ChunkVolume): CoreScan | null {
  const scanner = mesher === "wasm" ? wasm_scanner() : null;
  if (mesher === "wasm" && scanner === null) return null;
  if (scanner === null || !scanner.reaches(volume)) {
    if (scanner !== null) core_fallback_stats.unreached++;
    const job = ts_scan(volume);
    return job && { mesher, ...job };
  }
  const cursor = scanner.begin(volume);
  if (cursor === null) return null;
  return with_fallback(
    { advance: () => scanner.step(cursor), starved: () => cursor.starved, spilled: () => cursor.spilled, finish: () => scanner.finish(cursor), abort: () => scanner.abort(cursor) },
    () => ts_scan(volume),
  );
}

/**
 * Opens a bake of `set` at `step` on `mesher`, or null when that mesher's
 * one bake is taken — or, for the Rust mesher, while its module is still
 * arriving or has failed (resolve_core_mesher first). Either way the caller
 * is blocked and asks again next tick. A set the module cannot reach is
 * baked the TypeScript way whatever `mesher`.
 */
export function begin_core_bake(mesher: CoreMesher, set: ColumnRunSet, step: LodStep): CoreBake | null {
  const baker = mesher === "wasm" ? core_wasm_baker() : null;
  if (typeof baker === "string") return null;
  if (baker === null || !baker.reaches(set)) {
    if (baker !== null) core_fallback_stats.unreached++;
    const job = ts_bake(set, step);
    return job && { mesher, ...job };
  }
  const cursor = baker.begin(set, step);
  if (cursor === null) return null;
  return with_fallback(
    { advance: () => baker.step(cursor), starved: () => cursor.starved, spilled: () => cursor.spilled, finish: () => baker.finish(cursor), abort: () => baker.abort(cursor) },
    () => ts_bake(set, step),
  );
}
