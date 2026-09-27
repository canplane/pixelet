// @pxl/web/platform/web/core.ts

/**
 * The kernel as a page runs it: the Rust core's module (crates/pxl/web/wasm)
 * instantiated over a memory the central scheduler reserves. A device links
 * the core into its app instead and hands over the same two things — the
 * memory and the doorbell — so everything past this is the same on both.
 *
 * The bytes are built by `deno task build:wasm` into kernel/kernel.wasm and are
 * not committed; Vite hashes them into the bundle through the `new URL` below,
 * and Deno's tests read them through the same URL.
 *
 * The module imports its memory, and its first turn lays out everything inside
 * it — the rings, `.fixed` and the heap — as the kernel's map says
 * (crates/pxl/web/kernel/src/mem/section.rs). Past the memory it imports the
 * clock alone and exports the doorbell alone.
 *
 * One instance per memory: instantiating writes the module's data segments
 * over the front of it, so a second instance over the same memory would
 * erase the first's state — its heap's free lists among it.
 *
 * Instantiated from bytes rather than streamed: streaming insists on an
 * `application/wasm` response, which neither a `file:` URL under Deno nor
 * every static host gives.
 */

import type { f64, u32 } from "@xpute/core/abi/word.ts";
import { reserve_memory } from "@xpute/runtime/mem/reserve.ts";
import { DESKTOP_BYTES, PHONE_BYTES, REGION_BYTES } from "@/kernel/mem/mem.spec.ts";
import { BOOT, boot_block, type Core } from "@/kernel/module.ts";
import { run_gpu_stream } from "@/platform/web/webgpu.ts";
import { DEV_BUNDLE } from "@/config/bundle.ts";

/**
 * Whether this device is a phone or tablet, for the few ceilings that are
 * set by memory rather than by measurement of time.
 *
 * A coarse pointer is a touch screen, which is the one signal every mobile
 * browser gives and no desktop does; user agents lie and `deviceMemory` is
 * Chrome-only. What it gates: the core memory reservation, the shadow
 * map's size and the badge sheet's largest size — each a fixed
 * allocation a phone's tab cannot afford at the desktop's size. An iPhone
 * 13 mini read 1.6 to 2.1 GB for the page with them at the desktop's
 * numbers and reloaded under memory pressure (2026-09-13).
 */
export const HANDHELD: boolean = typeof matchMedia === "function" && matchMedia("(pointer: coarse)").matches;

/** The memory the kernel is reserved: a gibibyte on the desktop, half of one
 * on a phone. Both are mem.json's, the same file the kernel's map and the
 * module's linker flags read, so there is no second place to keep them true.
 *
 * There is no smaller reservation to fall back to. The terrain a view holds
 * is a fixed 24.62 MiB and the kernel lays its map out around it, so
 * a device that will not reserve this much cannot run the world at all — and
 * failing here says that, where failing in the map would say the map was
 * wrong. The module declares PHONE_BYTES as the floor it needs
 * (`--initial-memory`), so asking for less fails at instantiation too.
 *
 * A reservation is committed lazily, so it is sized for the peak;
 * JavaScriptCore nonetheless reports the whole of it as the page's JavaScript
 * memory (a Safari heap timeline read it as one ArrayBuffer), which is why a
 * phone gets half. */
const CORE_BYTES: u32 = HANDHELD ? PHONE_BYTES : DESKTOP_BYTES;

const KERNEL_WASM_URL = new URL("../../kernel/kernel.wasm", import.meta.url);

/** Reserves the memory and instantiates the module over it, booted. */
export async function open_core(): Promise<Core> {
  const memory = reserve_memory([CORE_BYTES / REGION_BYTES], REGION_BYTES);
  // Not from the HTTP cache in development: the dev server rebuilds the
  // module under the same URL, and a cached one silently runs yesterday's
  // core under today's glue.
  const bytes = await (await fetch(KERNEL_WASM_URL, DEV_BUNDLE ? { cache: "no-store" } : undefined)).arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {
    // The clock a tick's budget is spent against: the one call the module
    // makes, since a turn measures itself while the host cannot run.
    env: { memory, now: () => performance.now() },
  });
  const turn = instance.exports.interrupt as Core["interrupt"];
  // A turn with no quota applies what waits, which is nothing: the first one
  // lays the memory out and writes the boot block.
  turn(0);
  // The reservation's maximum is its initial size, so the buffer is never
  // replaced by a grow and one reference to it holds for the page's life.
  const buffer = memory.buffer as ArrayBuffer;
  // A module reaches WebGPU no other way than through the page, so the calls
  // its renderer recorded in a turn are made as the turn returns.
  const gpu = boot_block(buffer)[BOOT.gpu];
  const interrupt = (quota_ms: f64) => {
    const wake = turn(quota_ms);
    run_gpu_stream(buffer, gpu);
    return wake;
  };
  return { buffer, interrupt };
}
