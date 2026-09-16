// @pxl/web/wasm/core_memory.ts

/**
 * The central scheduler's big memory, and the kernel booted over it.
 *
 * How much memory the kernel gets is the central scheduler's: one fixed
 * `WebAssembly.Memory`, reserved and handed to the module
 * (@xpute/core/sys/memory.ts). A reservation is committed lazily, so it is
 * sized for the peak; JavaScriptCore nonetheless reports the whole of it as
 * the page's JavaScript memory (a Safari heap timeline read it as one
 * ArrayBuffer), which is why a phone gets half.
 *
 * What is where inside it is the kernel's alone (crates/pxl/core/src/memory.rs):
 * the rings sit at addresses its build.rs fixed, the fixed ranges follow
 * whatever the linker placed, and the heap is everything past them. The shell
 * holds no picture of any of it — a block is an address and a length the
 * kernel handed out.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import { reserve_memory } from "@xpute/core/sys/memory.ts";
import { HANDHELD } from "@/shared/lib/browser/handheld.ts";
import { DESKTOP_BYTES, PHONE_BYTES, REGION_BYTES } from "@/wasm/memory_spec.ts";
import { BOOT, boot_block, instantiate_kernel, type KernelExports } from "@/wasm/module.ts";
import { CoreRing } from "@/wasm/core_ring.ts";

export { REGION_BYTES };

/** The memory the kernel is reserved: a gibibyte on the desktop, half of one
 * on a phone. Both are memory.json's, the same file the kernel's map and the
 * module's linker flags read, so there is no second place to keep them true.
 *
 * There is no smaller reservation to fall back to. The terrain a load radius
 * reaches is a fixed 196.43 MiB and the kernel lays its map out around it, so
 * a device that will not reserve this much cannot run the world at all — and
 * failing here says that, where failing in the map would say the map was
 * wrong. The module declares PHONE_BYTES as the floor it needs
 * (`--initial-memory`), so asking for less fails at instantiation too. */
const CORE_BYTES: u32 = HANDHELD ? PHONE_BYTES : DESKTOP_BYTES;

const memory = reserve_memory([CORE_BYTES / REGION_BYTES], REGION_BYTES);

/**
 * The one module over it, booted: the kernel has laid the memory out before
 * anything here reads it, and nothing may run before it has arrived. A module
 * that fails to load fails the client — the memory exists for the kernel.
 */
export const CORE_KERNEL: KernelExports = await instantiate_kernel(memory);

export const CORE_MEMORY: WebAssembly.Memory = memory;

/** The module's own front, `[0, bytes)` of the memory: its stack, its rings
 * and everything the linker placed. Where it ends is the linker's answer and
 * not a number chosen in advance, so it moves when the module's code does. */
export const MODULE_SECTION: { readonly offset: u32; readonly bytes: u32 } = { offset: 0, bytes: boot_block(CORE_KERNEL)[BOOT.data_end] };

/** The rings to the module, made with it. */
export const CORE_RING: CoreRing = new CoreRing(CORE_KERNEL);
