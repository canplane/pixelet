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
 * Instantiated from bytes rather than streamed: streaming insists on an
 * `application/wasm` response, which neither a `file:` URL under Deno nor
 * every static host gives.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import { MODULE_SECTION } from "@/render/layer/pipeline/core_pool.ts";

/** What the module exports. Every address is into the memory it was
 * instantiated over. */
export type PxlExports = {
  memory: WebAssembly.Memory;
  pxl_module_bytes(): u32;
  pxl_bake_begin(at: u32, count: u32, span: u32, lod_step: u32): u32;
  pxl_bake_step(): u32;
  pxl_bake_mesh_words(): u32;
  pxl_bake_mesh(at: u32, words: u32): void;
  pxl_bake_finish(): u32;
  pxl_bake_abort(): void;
  pxl_scan_begin(bands_at: u32, count: u32): u32;
  pxl_scan_step(): u32;
  pxl_scan_set_bytes(): u32;
  pxl_scan_set(at: u32, bytes: u32): void;
  pxl_scan_finish(): u32;
  pxl_scan_abort(): void;
};

/** pxl_bake_step's answers (crates/pxl-wasm/src/lib.rs). */
export const STEP_WORKING: u32 = 0;
export const STEP_NEED_MESH: u32 = 1;
export const STEP_DONE: u32 = 2;

const PXL_WASM_URL = new URL("./pxl.wasm", import.meta.url);

export async function instantiate_pxl(memory: WebAssembly.Memory): Promise<PxlExports> {
  const bytes = await (await fetch(PXL_WASM_URL)).arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, { env: { memory } });
  const exports = instance.exports as unknown as PxlExports;
  const keeps = exports.pxl_module_bytes();
  if (keeps !== MODULE_SECTION.bytes) {
    throw new Error(`pxl: the module keeps ${keeps} bytes of the memory and the map reserves ${MODULE_SECTION.bytes}`);
  }
  return { ...exports, memory };
}
