// @pxl/web/wasm/pxl.ts

/**
 * The Rust core's module (crates/pxl-wasm), instantiated. The bytes are
 * built by `deno task build:wasm` into `pxl.wasm` beside this file and are
 * not committed; Vite hashes them into the bundle through the `new URL`
 * below, and Deno's tests read them through the same URL.
 *
 * Instantiated from bytes rather than streamed: streaming insists on an
 * `application/wasm` response, which neither a `file:` URL under Deno nor
 * every static host gives.
 */

import type { u32 } from "@xpute/core/kit/word.ts";

/** What the module exports. Addresses are into `memory`, and a call may
 * grow it — so a view is taken after the call that hands out an address,
 * never held across another. */
export type PxlExports = {
  memory: WebAssembly.Memory;
  pxl_bake_input(count: u32, span: u32): u32;
  pxl_bake_begin(lod_step: u32): u32;
  pxl_bake_step(): u32;
  pxl_bake_finish(): u32;
  pxl_bake_abort(): void;
};

const PXL_WASM_URL = new URL("./pxl.wasm", import.meta.url);

export async function instantiate_pxl(): Promise<PxlExports> {
  const bytes = await (await fetch(PXL_WASM_URL)).arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {});
  return instance.exports as unknown as PxlExports;
}
