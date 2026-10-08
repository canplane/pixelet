// @xpute/host/ipc/stream.ts

/**
 * A turn's calls from guest to host (rust/guest/src/ipc/stream.rs),
 * run after the turn returns. Word 0 of the head is END, the count of record
 * words past the head; a record is op, n, then n words.
 */

import type { U32Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { td } from "@xpute/kit/codec/encoding.ts";

export const END = 0;

/** The words are valid only inside `run`. */
export function each_record(memory: ArrayBufferLike, at: u32, head: u32, run: (op: u32, w: U32Array, a: u32, n: u32) => void): void {
  const end = new Uint32Array(memory, at, head)[END];
  if (end === 0) return;
  const w = new Uint32Array(memory, at + 4 * head, end) as U32Array;
  for (let k = 0; k < end;) {
    const [op, n] = [w[k], w[k + 1]];
    run(op, w, k + 2, n);
    k += 2 + n;
  }
}

export function bytes_of(w: U32Array, i: u32): Uint8Array {
  return new Uint8Array(w.buffer, w.byteOffset + 4 * (i + 1), w[i]);
}

export function text_of(w: U32Array, i: u32): string {
  return td.decode(bytes_of(w, i));
}
