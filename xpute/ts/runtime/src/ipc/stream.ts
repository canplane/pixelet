// @xpute/runtime/ipc/stream.ts

/**
 * A stream carries a turn's calls from the core to the runtime (xpute-core
 * ipc/stream.rs), which the runtime runs after the turn returns. The records
 * are found from the stream's own words (spec/ipc/stream.json).
 */

import type { U32Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { td } from "@xpute/kit/codec/encoding.ts";

import { END, HEAD } from "./stream.spec.ts";

/** Runs each record of the stream at `at` through `run`, given its op, the
 * stream's words, the index of its first word and its count. The words are
 * valid only inside `run`. Once this returns, even by a throw from `run`, the
 * stream is handed back to the core. */
export function each_rec(
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
