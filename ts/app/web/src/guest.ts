// @pxl/web/guest.ts

/**
 * The kernel as a page runs it: the guest's module opened over a reserved
 * memory (@xpute/host module.ts). One instance per memory, since
 * instantiating overwrites the data segments. Compiled from bytes, not
 * streamed: streaming needs an `application/wasm` response, which a `file:`
 * URL lacks.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { reserve_mem } from "@xpute/host/mem/reserve.ts";
import { open_module } from "@xpute/host/module.ts";
import { Directory } from "@xpute/host/ipc/directory.ts";
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
  // The first turn lays the memory out and writes the directory.
  guest.run(0);
  const gpu = new Directory(guest.mem, cfg.directory_at).get(cfg.gpu_stream);
  webgpu.take(guest.mem);
  return {
    mem: guest.mem,
    run: (quota_ms: f64) => {
      const wake = guest.run(quota_ms);
      webgpu.run(gpu, cfg.gpu_head);
      return wake;
    },
    answer_with: guest.answer_with,
  };
}
