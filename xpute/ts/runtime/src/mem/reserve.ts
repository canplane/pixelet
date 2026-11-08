// @xpute/runtime/mem/reserve.ts

/**
 * The core's memory, reserved with `initial == maximum` so it never grows and
 * `memory.buffer` never detaches: views over it stay valid for the module's life.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";

const WASM_PAGE_BYTES: u32 = 1 << 16;

/** Reserves the first of `candidates`, each a count of regions, that the
 * device will grant. It throws ENOMEM when it grants none. */
export function reserve_mem(
  candidates: readonly u32[],
  region_bytes: u32,
): WebAssembly.Memory {
  for (const regions of candidates) {
    const pages = (regions * region_bytes) / WASM_PAGE_BYTES;
    try {
      return new WebAssembly.Memory({ initial: pages, maximum: pages });
    } catch {
      // The memory is too large for this device; the next candidate is tried.
    }
  }
  throw new MarshalError(Errno.ENOMEM);
}
