// @xpute/core/ipc/ring.ts

/**
 * A single-producer, single-consumer ring of messages in a buffer both
 * sides can see — the submission and completion rings between a shell and
 * its kernel, laid out once and shared by address.
 *
 * A message is what a frame (frame.ts) needs once the transport is shared
 * memory rather than transfer: no spill, since a packet is as large as the
 * arena allows; no args/body split, since the packet is a tree and a large
 * body is a leaf of it; no worker id, since one ring joins two sides. What
 * is left is four words:
 *
 *   0  tag          the submitter's, echoed on the reply; on a signal, its
 *                   subject
 *   1  cmd | flags  cmd (cmd.ts: major << 8 | minor) in the low 16 bits,
 *                   FrameFlag in the high 16
 *   2  result       on a reply: a value, or a negative Errno; else 0
 *   3  packet       where the message's XTP packet starts, from the ring's
 *                   own start, or 0 for none — its length is in its header
 *
 * On the completion ring, `RES` tells a reply from a signal the kernel
 * raised itself. `ACKREQ` on a submission asks for exactly one reply.
 *
 * Layout: a header of eight u32 words, `capacity` messages, then the arena
 * the producer writes packets into, 8-aligned, one after another:
 *
 *   0  capacity     a power of two
 *   1  head         position of the next message to consume
 *   2  tail         position of the next message to produce
 *   3  arena_bytes  a multiple of 8
 *   4  arena_tail   where the producer's next packet goes, in the arena
 *   5…7 reserved
 *
 * Positions only grow and are taken modulo the capacity when read, so
 * `tail - head` is what waits. Each side writes only its own words — the
 * producer the tail and the arena, the consumer the head — so the two share
 * the ring without a lock. The producer rewinds the arena when it finds the
 * ring empty: every packet written before then belongs to a message already
 * consumed. A packet read by the consumer is valid until it advances past
 * the message.
 *
 * A full ring, or an arena without room, refuses a push (EAGAIN): the
 * producer waits, and nothing is dropped. A packet larger than the whole
 * arena can never fit and is refused as EMSGSIZE. The doorbell — how the
 * consumer is told there is something to read — is not part of the ring;
 * it is whatever call the two sides already take turns at.
 */

import type { i32, u32 } from "@xpute/core/kit/word.ts";
import type { U8Array } from "@xpute/core/kit/array.ts";
import { Errno } from "@xpute/core/sys/errno.ts";
import { MarshalError } from "@xpute/core/sys/error.ts";

export const RING_HEADER_WORDS: u32 = 8;
export const RING_ENTRY_WORDS: u32 = 4;

export const RING_TAG: u32 = 0;
export const RING_CMD: u32 = 1;
export const RING_RESULT: u32 = 2;
export const RING_PACKET: u32 = 3;

/** Bytes a ring of `capacity` messages and an arena of `arena_bytes` takes. */
export function ring_bytes(capacity: u32, arena_bytes: u32): u32 {
  return (RING_HEADER_WORDS + capacity * RING_ENTRY_WORDS) * 4 + arena_bytes;
}

/** A packet's bytes: its 16-byte header, then the payload its second word
 * names, to the word. */
function packet_bytes(u8: U8Array, at: u32): u32 {
  const payload = u8[at + 8] | (u8[at + 9] << 8) | (u8[at + 10] << 16) | (u8[at + 11] << 24);
  return Math.ceil((16 + (payload >>> 0)) / 8) * 8;
}

export class Ring {
  /** The header and the messages, as words; message `slot` starts at
   * `entry_at(slot)`. */
  readonly words: Uint32Array;
  readonly capacity: u32;
  readonly arena_bytes: u32;
  private readonly mask: u32;
  private readonly u8: U8Array;
  /** Where the arena starts, from the ring's start. */
  private readonly arena_at: u32;

  /** A ring already laid out at byte `offset` of `buffer`. */
  constructor(buffer: ArrayBufferLike, offset: u32) {
    const head = new Uint32Array(buffer, offset, RING_HEADER_WORDS);
    const capacity = head[0];
    if (capacity === 0 || (capacity & (capacity - 1)) !== 0) throw new Error(`ring: capacity ${capacity} at ${offset} is not a power of two`);
    this.capacity = capacity;
    this.arena_bytes = head[3];
    this.mask = capacity - 1;
    this.words = new Uint32Array(buffer, offset, RING_HEADER_WORDS + capacity * RING_ENTRY_WORDS);
    this.arena_at = this.words.byteLength;
    // The memory a ring lives in is never a SharedArrayBuffer here (the
    // core memory is fixed, not shared), and the packets it hands out are
    // read by the tree reader, which takes an ArrayBuffer view.
    this.u8 = new Uint8Array(buffer as ArrayBuffer, offset, ring_bytes(capacity, this.arena_bytes));
  }

  /** Lays out an empty ring of `capacity` messages and an arena of
   * `arena_bytes` at byte `offset`. */
  static init(buffer: ArrayBufferLike, offset: u32, capacity: u32, arena_bytes: u32): Ring {
    if (capacity === 0 || (capacity & (capacity - 1)) !== 0) throw new Error(`ring: capacity ${capacity} is not a power of two`);
    if (arena_bytes % 8 !== 0) throw new Error(`ring: an arena of ${arena_bytes} bytes is not whole words`);
    new Uint8Array(buffer, offset, ring_bytes(capacity, arena_bytes)).fill(0);
    const head = new Uint32Array(buffer, offset, RING_HEADER_WORDS);
    head[0] = capacity;
    head[3] = arena_bytes;
    return new Ring(buffer, offset);
  }

  get head(): u32 {
    return this.words[1];
  }

  get tail(): u32 {
    return this.words[2];
  }

  /** Messages waiting to be consumed. */
  get len(): u32 {
    return (this.words[2] - this.words[1]) >>> 0;
  }

  /** Where the message at `position` starts in `words`. */
  entry_at(position: u32): u32 {
    return RING_HEADER_WORDS + ((position & this.mask) >>> 0) * RING_ENTRY_WORDS;
  }

  // ---- producer ----

  /**
   * Writes one message at the tail, its packet copied into the arena.
   * EAGAIN, and nothing written, when the ring is full or the arena has no
   * room left; EMSGSIZE when the packet is larger than the whole arena.
   */
  push(tag: u32, cmd: u32, flags: u32 = 0, packet: U8Array | null = null, result: i32 = 0): Errno {
    if (this.len >= this.capacity) return Errno.EAGAIN;
    const w = this.words;
    if (this.len === 0) w[4] = 0;
    let packet_at = 0;
    if (packet !== null) {
      const need = Math.ceil(packet.byteLength / 8) * 8;
      if (need > this.arena_bytes) return Errno.EMSGSIZE;
      if (w[4] + need > this.arena_bytes) return Errno.EAGAIN;
      packet_at = this.arena_at + w[4];
      this.u8.set(packet, packet_at);
      this.u8.fill(0, packet_at + packet.byteLength, packet_at + need);
      w[4] += need;
    }
    const at = this.entry_at(w[2]);
    w[at + RING_TAG] = tag;
    w[at + RING_CMD] = ((cmd & 0xffff) | ((flags & 0xffff) << 16)) >>> 0;
    w[at + RING_RESULT] = result >>> 0;
    w[at + RING_PACKET] = packet_at;
    w[2] = (w[2] + 1) >>> 0;
    return Errno.OK;
  }

  // ---- consumer ----

  /** Where the message at the head starts in `words`, or -1 when empty. Read
   * it in place, then `advance`. */
  peek(): number {
    return this.len === 0 ? -1 : this.entry_at(this.words[1]);
  }

  tag(at: u32): u32 {
    return this.words[at + RING_TAG];
  }

  cmd(at: u32): u32 {
    return this.words[at + RING_CMD] & 0xffff;
  }

  flags(at: u32): u32 {
    return this.words[at + RING_CMD] >>> 16;
  }

  result(at: u32): i32 {
    return this.words[at + RING_RESULT] | 0;
  }

  /** The message's packet, a view into the arena valid until `advance`, or
   * null when it has none. EBADMSG when it names bytes outside the arena. */
  packet(at: u32): U8Array | null {
    const packet_at = this.words[at + RING_PACKET];
    if (packet_at === 0) return null;
    const end = this.arena_at + this.arena_bytes;
    if (packet_at < this.arena_at || packet_at + 16 > end || packet_bytes(this.u8, packet_at) > end - packet_at) {
      throw new MarshalError(Errno.EBADMSG, `ring: message packet at ${packet_at} lies outside the arena`);
    }
    return this.u8.subarray(packet_at, packet_at + packet_bytes(this.u8, packet_at));
  }

  /** Consumes the message at the head. */
  advance(): void {
    if (this.len === 0) throw new Error("ring: advance on an empty ring");
    this.words[1] = (this.words[1] + 1) >>> 0;
  }
}
