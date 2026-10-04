// @pxl/web/kernel/module.ts

/**
 * The Rust core as the shell holds it on any platform: its memory and its
 * doorbell. Everything else crosses in the memory.
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import type { f64, u32 } from "@xpute/core/abi/word.ts";
import { Directory } from "@xpute/runtime/ipc/directory.ts";

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

const utf8_decoder = new TextDecoder();

/** A range's bytes, where they lie. Valid until the kernel is given them back
 * or writes them again, so a caller that keeps them copies them first. */
export function bytes_at(memory: ArrayBuffer, at: u32, len: u32): U8Array {
  return new Uint8Array(memory, at, len);
}

/** A range's bytes as text, `len` bytes of UTF-8. */
export function text_at(memory: ArrayBuffer, at: u32, len: u32): string {
  return utf8_decoder.decode(bytes_at(memory, at, len));
}
