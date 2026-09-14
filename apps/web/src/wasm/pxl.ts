// @pxl/web/wasm/pxl.ts

/**
 * The Rust core's module (crates/pxl/wasm), instantiated over the core
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

import type { u32 } from "@xpute/core/kit/word.ts";

/** What the module exports. Every address is into the memory it was
 * instantiated over. */
export type PxlExports = {
  memory: WebAssembly.Memory;
  pxl_module_bytes(): u32;
  pxl_ring_sq(): u32;
  pxl_ring_cq(): u32;
  pxl_drain(): u32;
  pxl_tick(): u32;
};

/** A step's result, BAKE_STEP's and SCAN_STEP's (@pxl/shared/spec/cmd.ts). */
export const STEP_WORKING: u32 = 0;
export const STEP_NEED_MESH: u32 = 1;
export const STEP_DONE: u32 = 2;

const PXL_WASM_URL = new URL("./pxl.wasm", import.meta.url);

/** Instantiates the module over `memory`, whose first `module_bytes` are
 * the map's module section. */
export async function instantiate_pxl(memory: WebAssembly.Memory, module_bytes: u32): Promise<PxlExports> {
  // Not from the HTTP cache in development: the dev server rebuilds the
  // module under the same URL, and a cached one silently runs yesterday's
  // core under today's glue.
  const bytes = await (await fetch(PXL_WASM_URL, import.meta.env?.DEV ? { cache: "no-store" } : undefined)).arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {
    env: {
      memory,
      // A panic's message, before the trap: what "unreachable" would otherwise hide.
      pxl_panic: (ptr: u32, len: u32) => console.error(`[pxl] ${new TextDecoder().decode(new Uint8Array(memory.buffer, ptr, len))}`),
      // The clock a tick's budget is spent against.
      pxl_now: () => performance.now(),
    },
  });
  const exports = instance.exports as unknown as PxlExports;
  const keeps = exports.pxl_module_bytes();
  if (keeps !== module_bytes) {
    throw new Error(`pxl: the module keeps ${keeps} bytes of the memory and the map reserves ${module_bytes}`);
  }
  return { ...exports, memory };
}
