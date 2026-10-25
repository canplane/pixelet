// @pxl/web/guest.ts

/**
 * The kernel as a page runs it: the guest's module instantiated over a
 * reserved memory. One instance per memory, since instantiating overwrites
 * the data segments. Instantiated from bytes, not streamed: streaming needs
 * an `application/wasm` response, which a `file:` URL under Deno lacks.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { reserve_memory } from "@xpute/host/mem/reserve.ts";
import { Directory } from "@xpute/host/ipc/directory.ts";
import type { Guest, GuestPlan } from "@pxl/client/bracket.tsx";
import { runGpuStream } from "./webgpu.ts";
import { DEV_BUNDLE } from "@pxl/client/config/bundle.ts";

const GUEST_WASM_URL = new URL("./guest.wasm", import.meta.url);

/** No smaller fallback: the guest's map needs `plan.bytes`. */
export async function openGuest(plan: GuestPlan): Promise<Guest> {
  const memory = reserve_memory([plan.bytes / plan.region], plan.region);
  // A cached module would run yesterday's guest under today's glue.
  const bytes = await (await fetch(GUEST_WASM_URL, DEV_BUNDLE ? { cache: "no-store" } : undefined)).arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {
    env: { memory, now: () => performance.now() },
  });
  const turn = instance.exports.interrupt as Guest["interrupt"];
  // The first turn lays the memory out and writes the directory.
  turn(0);
  // The maximum is the initial size, so the buffer is never replaced.
  const buffer = memory.buffer as ArrayBuffer;
  const gpu = new Directory(buffer, plan.directory_at).get(plan.gpu_stream);
  const interrupt = (quotaMs: f64) => {
    const wake = turn(quotaMs);
    runGpuStream(buffer, gpu, plan.gpu_head);
    return wake;
  };
  return { buffer, interrupt };
}
