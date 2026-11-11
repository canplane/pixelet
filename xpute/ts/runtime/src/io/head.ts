// @xpute/runtime/io/head.ts

/**
 * The runtime lays the front's header (spec/io/head.json) in a core's memory
 * from the core's manifest before the first turn. It is read back to find the
 * GPU stream. On its first turn the core checks the header against its
 * manifest.
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import { MAGIC, WORDS, word } from "./head.spec.ts";

/** A `Front` is the front a manifest declares: the most a frame's packet
 * takes, and where the GPU stream lies, 0 without the GPU device. */
export interface Front {
  pkt_bytes: u32;
  stream_at: u32;
}

/** This says whether a header is laid at `at`. A core that outlived its
 * runtime's reload keeps the header laid for it. */
export function laid(mem: ArrayBufferLike, at: u32): boolean {
  return new Uint32Array(mem, at, WORDS)[word.MAGIC] === MAGIC;
}

/** Lays the header at `at`. */
export function lay(mem: ArrayBufferLike, at: u32, f: Front): void {
  const h = new Uint32Array(mem, at, WORDS);
  h[word.MAGIC] = MAGIC;
  h[word.PKT_BYTES] = f.pkt_bytes;
  h[word.STREAM_AT] = f.stream_at;
}

/** A `Head` says what a laid header holds. A header without xpute's MAGIC
 * throws EBADMSG. */
export class Head {
  readonly pkt_bytes: u32;
  readonly stream_at: u32;

  constructor(mem: ArrayBufferLike, at: u32) {
    const h = new Uint32Array(mem, at, WORDS);
    if (h[word.MAGIC] !== MAGIC) throw new MarshalError(Errno.EBADMSG);
    this.pkt_bytes = h[word.PKT_BYTES];
    this.stream_at = h[word.STREAM_AT];
  }
}
