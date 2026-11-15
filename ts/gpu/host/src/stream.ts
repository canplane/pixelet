// @pxl/gpu/host/src/stream.ts

/**
 * The stream's records as the host reads them from what the guest lent to
 * `gpu.run` (pxl-gpu-guest stream.rs): op, n, then n words; bytes go as their
 * count, then the bytes padded to a word.
 */

import type { U32Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { td } from "@xpute/kit/codec/encoding.ts";

/** Runs each record of `w` through `run`, given its op, the words, the index
 * of its first word and its count. */
export function each_rec(
  w: U32Array,
  run: (op: u32, w: U32Array, a: u32, n: u32) => void,
): void {
  for (let k = 0; k < w.length;) {
    const op = w[k]!;
    const n = w[k + 1]!;
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
