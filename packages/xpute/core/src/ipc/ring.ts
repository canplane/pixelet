// @xpute/core/ipc/ring.ts

/**
 * A single-producer, single-consumer ring of fixed-size entries in a buffer
 * both sides can see — the submission and completion rings between a glue
 * and its core, laid out once and shared by address.
 *
 * A ring is a header of four u32 words and then `capacity` entries of
 * RING_ENTRY_WORDS words:
 *
 *   0  capacity   a power of two
 *   1  head       position of the next entry to consume
 *   2  tail       position of the next entry to produce
 *   3  reserved
 *
 * Positions only grow and are taken modulo the capacity when read, so
 * `tail - head` is what waits and a full ring is one where that is the
 * capacity. Each side writes only its own word — the producer the tail, the
 * consumer the head — so the two share the ring without a lock.
 *
 * An entry mirrors the first words of a frame (frame.ts):
 *
 *   0  user       the submitter's, echoed back on the completion
 *   1  op | flags op in the low 16 bits
 *   2  result     on a completion: a value, or a negative Errno
 *   3  reserved
 *   4… args       twelve words; anything larger is a block in the buffer,
 *                 named by its offset
 *
 * A full ring refuses a push (EAGAIN): the producer waits, and nothing is
 * dropped. The doorbell — how the consumer is told there is something to
 * read — is not part of the ring; it is whatever call the two sides already
 * take turns at.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import { Errno } from "@xpute/core/sys/errno.ts";

export const RING_HEADER_WORDS: u32 = 4;
export const RING_ENTRY_WORDS: u32 = 16;
export const RING_ARG_WORDS: u32 = RING_ENTRY_WORDS - 4;

export const RING_USER: u32 = 0;
export const RING_OP: u32 = 1;
export const RING_RESULT: u32 = 2;
export const RING_ARGS: u32 = 4;

/** Words a ring of `capacity` entries takes. */
export function ring_words(capacity: u32): u32 {
  return RING_HEADER_WORDS + capacity * RING_ENTRY_WORDS;
}

export class Ring {
  /** The whole ring's words; entry `slot` starts at `entry_at(slot)`. */
  readonly words: Uint32Array;
  readonly capacity: u32;
  private readonly mask: u32;

  /** A ring already laid out at byte `offset` of `buffer`. */
  constructor(buffer: ArrayBufferLike, offset: u32) {
    const capacity = new Uint32Array(buffer, offset, 1)[0];
    if (capacity === 0 || (capacity & (capacity - 1)) !== 0) throw new Error(`ring: capacity ${capacity} at ${offset} is not a power of two`);
    this.capacity = capacity;
    this.mask = capacity - 1;
    this.words = new Uint32Array(buffer, offset, ring_words(capacity));
  }

  /** Lays out an empty ring of `capacity` entries at byte `offset`. */
  static init(buffer: ArrayBufferLike, offset: u32, capacity: u32): Ring {
    if (capacity === 0 || (capacity & (capacity - 1)) !== 0) throw new Error(`ring: capacity ${capacity} is not a power of two`);
    const words = new Uint32Array(buffer, offset, ring_words(capacity));
    words.fill(0);
    words[0] = capacity;
    return new Ring(buffer, offset);
  }

  get head(): u32 {
    return this.words[1];
  }

  get tail(): u32 {
    return this.words[2];
  }

  /** Entries waiting to be consumed. */
  get len(): u32 {
    return (this.words[2] - this.words[1]) >>> 0;
  }

  /** Where the entry at `position` starts in `words`. */
  entry_at(position: u32): u32 {
    return RING_HEADER_WORDS + ((position & this.mask) >>> 0) * RING_ENTRY_WORDS;
  }

  // ---- producer ----

  /** Writes one entry at the tail: `user`, `op`, and up to twelve args.
   * EAGAIN, and nothing written, when full. */
  push(user: u32, op: u32, ...args: u32[]): Errno {
    if (this.len >= this.capacity) return Errno.EAGAIN;
    if (args.length > RING_ARG_WORDS) throw new Error(`ring: ${args.length} args, at most ${RING_ARG_WORDS}`);
    const tail = this.words[2];
    const at = this.entry_at(tail);
    const w = this.words;
    w.fill(0, at, at + RING_ENTRY_WORDS);
    w[at + RING_USER] = user;
    w[at + RING_OP] = op;
    for (let k = 0; k < args.length; k++) w[at + RING_ARGS + k] = args[k];
    w[2] = (tail + 1) >>> 0;
    return Errno.OK;
  }

  // ---- consumer ----

  /** Where the entry at the head starts in `words`, or -1 when empty. Read
   * it in place, then `advance`. */
  peek(): number {
    return this.len === 0 ? -1 : this.entry_at(this.words[1]);
  }

  /** Consumes the entry at the head. */
  advance(): void {
    if (this.len === 0) throw new Error("ring: advance on an empty ring");
    this.words[1] = (this.words[1] + 1) >>> 0;
  }
}
