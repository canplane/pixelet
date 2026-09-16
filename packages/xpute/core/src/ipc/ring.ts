// @xpute/core/ipc/ring.ts

/**
 * A single-producer, single-consumer ring of messages in a buffer both
 * sides can see — the submission and completion rings between a shell and
 * its kernel, laid out once and shared by address.
 *
 * A message is what a frame (frame.ts) needs once the transport is shared
 * memory rather than transfer: no spill, since a packet is as large as a
 * frame allows; no args/body split, since the packet is a tree and a large
 * body is a leaf of it; no worker id, since one ring joins two sides. What
 * is left is four words:
 *
 *   0  tag          the submitter's, echoed on the reply; on a signal, its
 *                   subject
 *   1  cmd | flags  cmd (cmd.ts: major << 8 | minor) in the low 16 bits,
 *                   FrameFlag in the high 16
 *   2  result       on a reply: a value, or a negative Errno; else 0
 *   3  packet       where the message's XTP packet starts, an address in
 *                   the memory the two sides share, or 0 for none — its
 *                   length is in its header
 *
 * On the completion ring, `RES` tells a reply from a signal the kernel
 * raised itself. `ACKREQ` on a submission asks for exactly one reply.
 *
 * ## Two areas, not one
 *
 * The descriptors are a header of eight u32 words and `capacity` messages;
 * the packets live in an arena elsewhere in the memory, `capacity` frames of
 * `frame_bytes` each. Which is how every ring this is modelled on is built:
 * virtio's descriptor table, avail and used are three separate areas, and
 * AF_XDP's rings are one mapping and its UMEM another. The descriptors are
 * kilobytes and the arena is megabytes, so tying the two into one block makes
 * the small one as coarse as the large one.
 *
 *   0  capacity     a power of two
 *   1  head         position of the next message to consume
 *   2  tail         position of the next message to produce
 *   3  frame_bytes  a power of two, and a multiple of 8
 *   4  arena_at     where the frames begin
 *   5…7 reserved
 *
 * ## A frame a slot
 *
 * Message `n` writes its packet into frame `n & (capacity - 1)` — the frame
 * of the slot the message itself takes. A slot holds one live message, so its
 * frame is free exactly when it is, and there is no second bookkeeping to
 * keep: no arena cursor, no rewind, no moment at which the producer has to
 * find the ring empty before it can reuse space. AF_XDP's UMEM is the same
 * correspondence — a descriptor and the frame it names.
 *
 * What that buys is one failure mode. A push fails when the ring is full
 * (EAGAIN) and for no other reason, so the producer waits and nothing is
 * dropped. A packet larger than a frame is EMSGSIZE and is refused the same
 * way whatever the ring's state — where it once depended on how much of a
 * shared arena happened to be left, so the same packet went through or did
 * not depending on what had been sent before it.
 *
 * Positions only grow and are taken modulo the capacity when read, so
 * `tail - head` is what waits. Each side writes only its own words — the
 * producer the tail and its frames, the consumer the head — so the two share
 * the ring without a lock. A packet read by the consumer is valid until it
 * advances past the message.
 *
 * The doorbell — how the consumer is told there is something to read — is not
 * part of the ring; it is whatever call the two sides already take turns at.
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

/** Bytes the descriptors of a ring of `capacity` messages take. */
export function ring_bytes(capacity: u32): u32 {
  return (RING_HEADER_WORDS + capacity * RING_ENTRY_WORDS) * 4;
}

/** Bytes the arena of a ring of `capacity` frames of `frame_bytes` takes. */
export function arena_bytes(capacity: u32, frame_bytes: u32): u32 {
  return capacity * frame_bytes;
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
  readonly frame_bytes: u32;
  private readonly mask: u32;
  /** The frames, `capacity` of them, wherever in the memory they were put. */
  private readonly arena: U8Array;
  /** Where the frames begin, for the packet word to name one. */
  private readonly arena_at: u32;

  /** A ring already laid out: its descriptors at byte `at` of `mem`, its
   * arena where its header says. */
  constructor(mem: ArrayBufferLike, at: u32) {
    const head = new Uint32Array(mem, at, RING_HEADER_WORDS);
    const capacity = head[0];
    if (capacity === 0 || (capacity & (capacity - 1)) !== 0) throw new Error(`ring: capacity ${capacity} at ${at} is not a power of two`);
    this.capacity = capacity;
    this.frame_bytes = head[3];
    this.arena_at = head[4];
    this.mask = capacity - 1;
    this.words = new Uint32Array(mem, at, RING_HEADER_WORDS + capacity * RING_ENTRY_WORDS);
    // The memory a ring lives in is never a SharedArrayBuffer here (the core
    // memory is fixed, not shared), and the packets it hands out are read by
    // the tree reader, which takes an ArrayBuffer view.
    this.arena = new Uint8Array(mem as ArrayBuffer, this.arena_at, arena_bytes(capacity, this.frame_bytes));
  }

  /** Lays out an empty ring: `capacity` descriptors at byte `at` of `mem`,
   * and as many frames of `frame_bytes` at `arena_at`. */
  static init(mem: ArrayBufferLike, at: u32, capacity: u32, arena_at: u32, frame_bytes: u32): Ring {
    if (capacity === 0 || (capacity & (capacity - 1)) !== 0) throw new Error(`ring: capacity ${capacity} is not a power of two`);
    if (frame_bytes === 0 || (frame_bytes & (frame_bytes - 1)) !== 0 || frame_bytes % 8 !== 0) {
      throw new Error(`ring: a frame of ${frame_bytes} bytes is not a power of two of whole words`);
    }
    new Uint8Array(mem as ArrayBuffer, at, ring_bytes(capacity)).fill(0);
    const head = new Uint32Array(mem, at, RING_HEADER_WORDS);
    head[0] = capacity;
    head[3] = frame_bytes;
    head[4] = arena_at;
    return new Ring(mem, at);
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

  /** Where the frame of the message at `position` starts in the arena. One
   * frame a slot, so it is free exactly when the slot is. */
  private frame_at(position: u32): u32 {
    return ((position & this.mask) >>> 0) * this.frame_bytes;
  }

  // ---- producer ----

  /**
   * Writes one message at the tail, its packet copied into the frame of the
   * slot it takes. EAGAIN, and nothing written, when the ring is full;
   * EMSGSIZE when the packet is larger than a frame.
   */
  push(tag: u32, cmd: u32, flags: u32 = 0, packet: U8Array | null = null, result: i32 = 0): Errno {
    if (this.len >= this.capacity) return Errno.EAGAIN;
    const w = this.words;
    let packet_at = 0;
    if (packet !== null) {
      const need = Math.ceil(packet.byteLength / 8) * 8;
      if (need > this.frame_bytes) return Errno.EMSGSIZE;
      const start = this.frame_at(w[2]);
      this.arena.set(packet, start);
      this.arena.fill(0, start + packet.byteLength, start + need);
      packet_at = this.arena_at + start;
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

  /** The message's packet, a view into its frame valid until `advance`, or
   * null when it has none. EBADMSG when it names bytes outside a frame. */
  packet(at: u32): U8Array | null {
    const packet_at = this.words[at + RING_PACKET];
    if (packet_at === 0) return null;
    const off = packet_at - this.arena_at;
    if (off < 0 || off % this.frame_bytes !== 0 || off + 16 > this.arena.byteLength || packet_bytes(this.arena, off) > this.frame_bytes) {
      throw new MarshalError(Errno.EBADMSG, `ring: message packet at ${packet_at} lies outside a frame`);
    }
    return this.arena.subarray(off, off + packet_bytes(this.arena, off));
  }

  /** Consumes the message at the head. */
  advance(): void {
    if (this.len === 0) throw new Error("ring: advance on an empty ring");
    this.words[1] = (this.words[1] + 1) >>> 0;
  }
}
