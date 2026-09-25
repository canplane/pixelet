// @xpute/runtime/ipc/stream.ts

/**
 * A turn's calls from a guest to its host, as records in one range of the
 * memory both sides see (crates/xpute/runtime/src/ipc/stream.rs): the guest
 * writes them during a turn, and the host runs them here once the turn has
 * returned and before the next begins.
 *
 * The range opens on `head` words, of which word 0 is END: how many words of
 * records the turn wrote. Past it, a record is its op, the count of the words
 * that follow, and those words.
 *
 *   0      END      words of records past the head
 *   1…     the user's
 *   head…  op, n, n words; op, n, n words; …
 */

import type { U32Array } from "@xpute/core/abi/array.ts";
import type { u32 } from "@xpute/core/abi/word.ts";

/** The head word holding how many words of records the turn wrote. */
export const END = 0;

/** Calls `run` with each record the turn left in the stream at `at`: its op,
 * the records' words, and where its own words begin in them and how many
 * there are. The words are good only inside the call. */
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

/** A record's bytes, which open on their count at word `i` of `w`. */
export function bytes_of(w: U32Array, i: u32): Uint8Array {
  return new Uint8Array(w.buffer, w.byteOffset + 4 * (i + 1), w[i]);
}
