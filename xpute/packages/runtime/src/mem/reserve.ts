// @xpute/runtime/mem/reserve.ts

/**
 * The guest's memory, reserved with `initial == maximum` so it never grows and
 * `memory.buffer` never detaches: views over it stay valid for the module's life.
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import { MarshalError } from "@xpute/core/status/error.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";

const WASM_PAGE_BYTES: u32 = 1 << 16;

/** The first of `candidates` the device will reserve. */
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
