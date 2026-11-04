// @pxl/web/guest.ts

/**
 * The kernel as a page runs it: the guest's module opened over a reserved
 * memory (@xpute/host module.ts). One instance per memory, since
 * instantiating overwrites the data segments. Compiled from bytes, not
 * streamed: streaming needs an `application/wasm` response, which a `file:`
 * URL lacks.
 */

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { open_module, reserve_mem, Toc } from "@xpute/host";
import type { Config, Guest } from "@pxl/client/bracket.tsx";
import * as webgpu from "./webgpu.ts";
import { DEV_BUNDLE } from "./bundle.ts";

const GUEST_WASM_URL = new URL("./guest.wasm", import.meta.url);

/** No smaller fallback: the guest's map needs `cfg.bytes`. */
export async function open(cfg: Config): Promise<Guest> {
  const memory = reserve_mem([cfg.bytes / cfg.region], cfg.region);
  // A cached module would run yesterday's guest under today's glue.
  const bytes = await (
    await fetch(GUEST_WASM_URL, DEV_BUNDLE ? { cache: "no-store" } : undefined)
  ).arrayBuffer();
  const guest = open_module(await WebAssembly.compile(bytes), memory);
  webgpu.take(guest.mem);
  // Found once the first turn, which the mount runs, has written the toc.
  let gpu: u32 | null = null;
  return {
    mem: guest.mem,
    run: (quota_ms: f64) => {
      const wake = guest.run(quota_ms);
      gpu ??= new Toc(guest.mem, cfg.toc_at).get(cfg.gpu_stream);
      webgpu.run(gpu, cfg.gpu_head);
      return wake;
    },
    answer_with: guest.answer_with,
  };
}
