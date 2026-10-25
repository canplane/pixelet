// @pxl/client/host/module.ts

/**
 * The guest as the host holds it on any platform: its memory and its
 * doorbell. Everything else crosses in the memory.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { Directory } from "@xpute/host/ipc/directory.ts";

/** The memory never moves or grows. */
export interface Guest {
  buffer: ArrayBuffer;
  /** Returns when the guest wants its next turn, in ms. A guest that records
   * GPU calls runs them here, before the turn is over. */
  interrupt(quota_ms: f64): f64;
}

/** The directory the guest writes on its first turn, at the one offset the
 * host learns outside it. */
export function directory(memory: ArrayBuffer, at: u32): Directory {
  return new Directory(memory, at);
}

/** Valid until the guest reuses the range; copy to keep. */
export function bytes_at(memory: ArrayBuffer, at: u32, len: u32): U8Array {
  return new Uint8Array(memory, at, len);
}
