// @xpute/runtime/ipc/stream.ts

/**
 * A turn's calls from core to runtime (rust/core/src/ipc/stream.rs),
 * run after the turn returns. Word END of the head is the count of record
 * words past the head, and word HEAD the head's own count of words, so a runtime
 * finds the records from the stream itself; a record is op, n, then n words.
 * The core publishes END with release; the runtime loads it with acquire and,
 * once it has run the records, stores 0 with release, which hands the
 * stream back to the core for its next records.
 */

import type { U32Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { td } from "@xpute/kit/codec/encoding.ts";

export const END = 0;
export const HEAD = 1;

/** The words are valid only inside `run`, and the stream is the core's
 * again once this returns, a throw from `run` included. */
export function each_record(
  mem: ArrayBufferLike,
  at: u32,
  run: (op: u32, w: U32Array, a: u32, n: u32) => void,
): void {
  const own = new Uint32Array(mem, at, 2);
  const end = Atomics.load(own, END);
  if (end === 0) return;
  const w = new Uint32Array(mem, at + 4 * own[HEAD], end) as U32Array;
  try {
    for (let k = 0; k < end;) {
      const op = w[k];
      const n = w[k + 1];
      run(op, w, k + 2, n);
      k += 2 + n;
    }
  } finally {
    Atomics.store(own, END, 0);
  }
}

export function bytes_of(w: U32Array, i: u32): Uint8Array {
  return new Uint8Array(w.buffer, w.byteOffset + 4 * (i + 1), w[i]);
}

export function text_of(w: U32Array, i: u32): string {
  return td.decode(bytes_of(w, i));
}
