// @pxl/web/guest.ts

/**
 * The kernel as a page runs it: the guest's module instantiated over a
 * reserved memory. One instance per memory, since instantiating overwrites
 * the data segments. Instantiated from bytes, not streamed: streaming needs
 * an `application/wasm` response, which a `file:` URL under Deno lacks.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { reserve_mem } from "@xpute/host/mem/reserve.ts";
import { Directory } from "@xpute/host/ipc/directory.ts";
import type { Config, Guest } from "@pxl/client/bracket.tsx";
import * as webgpu from "./webgpu.ts";
import { DEV_BUNDLE } from "./bundle.ts";

const GUEST_WASM_URL = new URL("./guest.wasm", import.meta.url);

/** No smaller fallback: the guest's map needs `cfg.bytes`. */
export async function open(cfg: Config): Promise<Guest> {
  const memory = reserve_mem([cfg.bytes / cfg.region], cfg.region);
  // A cached module would run yesterday's guest under today's glue.
  const bytes = await (await fetch(
    GUEST_WASM_URL,
    DEV_BUNDLE ? { cache: "no-store" } : undefined,
  )).arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {
    env: { memory, now: () => performance.now() },
  });
  const turn = instance.exports.intr as Guest["intr"];
  // The first turn lays the memory out and writes the directory.
  turn(0);
  // The maximum is the initial size, so the buffer is never replaced.
  const mem = memory.buffer as ArrayBuffer;
  const gpu = new Directory(mem, cfg.directory_at).get(cfg.gpu_stream);
  webgpu.take(mem);
  const intr = (quota_ms: f64) => {
    const wake = turn(quota_ms);
    webgpu.run(gpu, cfg.gpu_head);
    return wake;
  };
  return { mem, intr };
}
