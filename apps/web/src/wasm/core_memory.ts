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
 * the sections, their pools and the rings are laid out when the module boots,
 * and the shell holds no picture of them — a block is an address and a length
 * the kernel handed out.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import { reserve_memory } from "@xpute/core/sys/memory.ts";
import { HANDHELD } from "@/shared/lib/browser/handheld.ts";
import { BOOT, boot_block, instantiate_kernel, type KernelExports } from "@/wasm/module.ts";
import { CoreRing } from "@/wasm/core_ring.ts";

/** crates/pxl/core/src/memory.rs's REGION_BYTES. */
export const REGION_BYTES: u32 = 1 << 26;

/** Regions the kernel is given: eight on the desktop, four on a phone — or
 * half where the device will not reserve that much, no fewer than the three
 * the kernel lays out. */
const CORE_REGIONS: u32 = HANDHELD ? 4 : 8;

const memory = reserve_memory([CORE_REGIONS, Math.max(3, Math.ceil(CORE_REGIONS / 2))], REGION_BYTES);

/**
 * The one module over it, booted: the kernel has laid the memory out before
 * anything here reads it, and nothing may run before it has arrived. A module
 * that fails to load fails the client — the memory exists for the kernel.
 */
export const CORE_KERNEL: KernelExports = await instantiate_kernel(memory);

export const CORE_MEMORY: WebAssembly.Memory = memory;

/** The module's own section, `[0, bytes)` of the memory. */
export const MODULE_SECTION: { readonly offset: u32; readonly bytes: u32 } = { offset: 0, bytes: boot_block(CORE_KERNEL)[BOOT.module_bytes] };

/** The rings to the module, made with it. */
export const CORE_RING: CoreRing = new CoreRing(CORE_KERNEL);
