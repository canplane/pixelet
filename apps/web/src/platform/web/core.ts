// @pxl/web/platform/web/core.ts

/**
 * The kernel as a page runs it: the Rust core's module instantiated over a
 * reserved memory. One instance per memory, since instantiating overwrites
 * the data segments. Instantiated from bytes, not streamed: streaming needs
 * an `application/wasm` response, which a `file:` URL under Deno lacks.
 */

import type { f64, u32 } from "@xpute/core/abi/word.ts";
import { reserve_memory } from "@xpute/host/mem/reserve.ts";
import { DESKTOP_BYTES, PHONE_BYTES, REGION_BYTES } from "@/kernel/mem/mem.spec.ts";
import { key } from "@/kernel/abi/directory.spec.ts";
import { type Core, directory } from "@/kernel/module.ts";
import { runGpuStream } from "@/platform/web/webgpu.ts";
import { DEV_BUNDLE } from "@/config/bundle.ts";

/** A coarse pointer: the one signal every mobile browser gives. Gates the
 * fixed allocations an iPhone 13 mini reloaded under at desktop sizes. */
export const HANDHELD: boolean = typeof matchMedia === "function" && matchMedia("(pointer: coarse)").matches;

/** No smaller fallback: the kernel's map needs this much. A phone gets half
 * because JavaScriptCore counts the whole reservation as page memory. */
const CORE_BYTES: u32 = HANDHELD ? PHONE_BYTES : DESKTOP_BYTES;

const KERNEL_WASM_URL = new URL("../../kernel/kernel.wasm", import.meta.url);

export async function openCore(): Promise<Core> {
  const memory = reserve_memory([CORE_BYTES / REGION_BYTES], REGION_BYTES);
  // A cached module would run yesterday's core under today's glue.
  const bytes = await (await fetch(KERNEL_WASM_URL, DEV_BUNDLE ? { cache: "no-store" } : undefined)).arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {
    env: { memory, now: () => performance.now() },
  });
  const turn = instance.exports.interrupt as Core["interrupt"];
  // The first turn lays the memory out and writes the directory.
  turn(0);
  // The maximum is the initial size, so the buffer is never replaced.
  const buffer = memory.buffer as ArrayBuffer;
  const gpu = directory(buffer).get(key.GPU_STREAM);
  const interrupt = (quotaMs: f64) => {
    const wake = turn(quotaMs);
    runGpuStream(buffer, gpu);
    return wake;
  };
  return { buffer, interrupt };
}
