// @xpute/runtime/mem/reserve.ts

/**
 * The memory a kernel is run in, reserved by the host.
 *
 * The central scheduler reserves one `WebAssembly.Memory` with
 * `initial == maximum`, which nothing grows, and gives it to the kernel. So
 * `memory.buffer` is one `ArrayBuffer` for the module's life: a view over it
 * stays valid, which is what lets a ring hold its words and a lane be read
 * where it lies. How the kernel lays its ranges out in it is the guest's
 * (xpute-runtime mem/section.rs), and nothing here reads it.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import { MarshalError } from "@xpute/core/status/error.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";

/** A WebAssembly page. */
const WASM_PAGE_BYTES: u32 = 1 << 16;

/**
 * The central scheduler's reservation: a memory of the first region count in
 * `candidates` the device will reserve, `initial == maximum` so that what it
 * hands back never grows and never detaches.
 */
export function reserve_memory(candidates: readonly u32[], region_bytes: u32): WebAssembly.Memory {
  for (const regions of candidates) {
    const pages = (regions * region_bytes) / WASM_PAGE_BYTES;
    try {
      return new WebAssembly.Memory({ initial: pages, maximum: pages });
    } catch {
      // too large for this device; try the next
    }
  }
  throw new MarshalError(Errno.ENOMEM);
}
