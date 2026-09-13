// @pxl/web/wasm/pxl.ts

/**
 * The Rust core's module (crates/pxl-wasm), instantiated over the core
 * memory. The bytes are built by `deno task build:wasm` into `pxl.wasm`
 * beside this file and are not committed; Vite hashes them into the bundle
 * through the `new URL` below, and Deno's tests read them through the same
 * URL.
 *
 * The module imports its memory rather than owning one, and keeps the
 * front of it — the map's module section — for its own stack, data and
 * heap. It says how much it keeps, and instantiation fails here rather
 * than later if the map disagrees: the sections after it would be under
 * the module's stack.
 *
 * One instance per memory, and for the core memory it is core_pool.ts's,
 * made before anything else uses the memory: instantiating writes the
 * module's data segments over the module section, so a second instance
 * over the same memory would erase the first's state — its pools' free
 * lists among it.
 *
 * Instantiated from bytes rather than streamed: streaming insists on an
 * `application/wasm` response, which neither a `file:` URL under Deno nor
 * every static host gives.
 */

import type { i32, u32 } from "@xpute/core/kit/word.ts";

/** What the module exports. Every address is into the memory it was
 * instantiated over. */
export type PxlExports = {
  memory: WebAssembly.Memory;
  pxl_module_bytes(): u32;
  pxl_heap_used(): u32;
  pxl_bake_begin(at: u32, count: u32, span: u32, lod_step: u32): u32;
  pxl_bake_step(): u32;
  pxl_bake_mesh_words(): u32;
  pxl_bake_mesh(at: u32, words: u32): void;
  pxl_bake_mesh_staged(words: u32): u32;
  pxl_bake_finish(): u32;
  pxl_bake_abort(): void;
  pxl_scan_begin(bands_at: u32, count: u32): u32;
  pxl_scan_step(): u32;
  pxl_scan_set_bytes(): u32;
  pxl_scan_set(at: u32, bytes: u32): void;
  pxl_scan_set_staged(bytes: u32): u32;
  pxl_scan_finish(): u32;
  pxl_scan_abort(): void;
  pxl_pool_new(start: u32, bytes: u32, region_bytes: u32, min_block_bytes: u32): u32;
  pxl_pool_acquire(id: u32, bytes: u32): u32;
  pxl_pool_release(id: u32, at: u32, bytes: u32): u32;
  pxl_pool_is_free(id: u32, at: u32, bytes: u32): u32;
  pxl_pool_stat(id: u32): u32;
  pxl_volume_pools(fixed: u32, mesh: u32): void;
  pxl_volume_open(): u32;
  pxl_volume_fill_column(id: u32, cx: u32, cy: u32, z0: i32, z1: i32, material: u32, layer_idx: u32): u32;
  pxl_volume_set_voxel(id: u32, cx: u32, cy: u32, z: i32, material: u32, layer_idx: u32, sub: u32): u32;
  pxl_volume_append_sub_voxel(id: u32, cx: u32, cy: u32, z: i32, material: u32, layer_idx: u32, sub: u32): u32;
  pxl_volume_seal(id: u32): void;
  pxl_volume_release(id: u32): void;
  pxl_volume_bands(id: u32): u32;
  pxl_volume_band(id: u32, i: u32): u32;
  pxl_volume_stat(): u32;
  pxl_scan_begin_volume(id: u32): u32;
};

/** pxl_bake_step's answers (crates/pxl-wasm/src/lib.rs). */
export const STEP_WORKING: u32 = 0;
export const STEP_NEED_MESH: u32 = 1;
export const STEP_DONE: u32 = 2;

const PXL_WASM_URL = new URL("./pxl.wasm", import.meta.url);

/** Instantiates the module over `memory`, whose first `module_bytes` are
 * the map's module section. */
export async function instantiate_pxl(memory: WebAssembly.Memory, module_bytes: u32): Promise<PxlExports> {
  const bytes = await (await fetch(PXL_WASM_URL)).arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {
    env: {
      memory,
      // A panic's message, before the trap: what "unreachable" would otherwise hide.
      pxl_panic: (ptr: u32, len: u32) => console.error(`[pxl] ${new TextDecoder().decode(new Uint8Array(memory.buffer, ptr, len))}`),
    },
  });
  const exports = instance.exports as unknown as PxlExports;
  const keeps = exports.pxl_module_bytes();
  if (keeps !== module_bytes) {
    throw new Error(`pxl: the module keeps ${keeps} bytes of the memory and the map reserves ${module_bytes}`);
  }
  return { ...exports, memory };
}
