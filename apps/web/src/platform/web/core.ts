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

import type { u32 } from "@xpute/core/abi/word.ts";
import { reserve_memory } from "@xpute/runtime/mem/section.ts";
import { HANDHELD } from "@/shared/lib/browser/handheld.ts";
import { DESKTOP_BYTES, PHONE_BYTES, REGION_BYTES } from "@/kernel/mem/mem.spec.ts";
import type { Core } from "@/kernel/module.ts";
import { DEV_BUNDLE } from "@/config/bundle.ts";

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
  const interrupt = instance.exports.interrupt as Core["interrupt"];
  // A turn with no quota applies what waits, which is nothing: the first one
  // lays the memory out and writes the boot block.
  interrupt(0);
  // The reservation's maximum is its initial size, so the buffer is never
  // replaced by a grow and one reference to it holds for the page's life.
  return { buffer: memory.buffer as ArrayBuffer, interrupt };
}
