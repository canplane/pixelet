// @pxl/web/kernel/module.ts

/**
 * The Rust core as the shell holds it on any platform: its memory and its
 * doorbell. Everything else crosses in the memory.
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import type { f64, u32 } from "@xpute/core/abi/word.ts";
import { Directory } from "@xpute/host/ipc/directory.ts";

import { DIRECTORY_AT } from "@/kernel/mem/mem.spec.ts";

/** The memory never moves or grows. */
export interface Core {
  buffer: ArrayBuffer;
  /** Returns when the kernel wants its next turn, in ms. A core that records
   * GPU calls runs them here, before the turn is over. */
  interrupt(quota_ms: f64): f64;
}

/** The directory the kernel writes on its first turn, at mem.json's DIRECTORY_AT. */
export function directory(memory: ArrayBuffer): Directory {
  return new Directory(memory, DIRECTORY_AT);
}

/** Valid until the kernel reuses the range; copy to keep. */
export function bytes_at(memory: ArrayBuffer, at: u32, len: u32): U8Array {
  return new Uint8Array(memory, at, len);
}
