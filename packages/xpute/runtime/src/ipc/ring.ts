// @xpute/runtime/ipc/ring.ts

/**
 * A single-producer, single-consumer ring of messages in a buffer both
 * sides can see — the submission and completion rings between a shell and
 * its kernel, laid out once and shared by address.
 *
 * A message is a frame (frame.ts), and this is where its four words sit: a
 * slot's descriptor, with word 3 the address of that slot's payload in the
 * memory both sides see. Nothing is added to it here — a packet is as large
 * as a slot allows, so there is no spill, and one ring joins two sides, so
 * `tag` carries no sender.
 *
 * ## A slot in two places
 *
 * A slot is a descriptor of four words and a payload of `slot_bytes`, and the
 * two lie apart because one is sixteen bytes and the other is thousands: the
 * descriptors are a header of eight u32 words and `capacity` of them, and the
 * payloads are `capacity` runs of `slot_bytes` elsewhere in the memory. Which is how every ring this is modelled on is built:
 * virtio's descriptor table, avail and used are three separate areas, and
 * AF_XDP's rings are one mapping and its UMEM another. The descriptors are
 * kilobytes and the payloads are megabytes, so tying the two into one block
 * makes the small one as coarse as the large one.
 *
 *   0  capacity     a power of two
 *   1  head         position of the next message to consume
 *   2  tail         position of the next message to produce
 *   3  slot_bytes   a power of two, and a multiple of 8
 *   4  slot_base    where the payloads begin
 *   5…7 reserved
 *
 * ## One payload a slot
 *
 * Message `n` takes slot `n & (capacity - 1)` and writes its packet into that
 * slot's payload — `entry_at` and `slot_at` answer the same `position`. A slot
 * holds one live message, so its payload is free exactly when it is, and there
 * is no second bookkeeping to keep: no cursor over a shared area, no rewind,
 * no moment at which the producer has to find the ring empty before it can
 * reuse space. AF_XDP's UMEM is the same correspondence — a descriptor and the
 * buffer it names.
 *
 * What that buys is one failure mode. A push fails when the ring is full
 * (EAGAIN) and for no other reason, so the producer waits and nothing is
 * dropped. A packet larger than a slot is EMSGSIZE and is refused the same
 * way whatever the ring's state — where it once depended on how much of a
 * shared area happened to be left, so the same packet went through or did not
 * depending on what had been sent before it.
 *
 * Positions only grow and are taken modulo the capacity when read, so
 * `tail - head` is what waits. Each side writes only its own words — the
 * producer the tail and its payloads, the consumer the head — so the two share
 * the ring without a lock. A packet read by the consumer is valid until it
 * advances past the message.
 *
 * The doorbell — how the consumer is told there is something to read — is not
 * part of the ring; it is whatever call the two sides already take turns at.
 */

import type { i32, u32 } from "@xpute/core/abi/word.ts";
import type { U8Array } from "@xpute/core/abi/array.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { MarshalError } from "@xpute/core/status/error.ts";

import { cmd_of, cmd_word, flags_of, FRAME_CMD, FRAME_PACKET, FRAME_RESULT, FRAME_TAG, FRAME_WORDS } from "@xpute/runtime/ipc/frame.ts";

export const RING_HEADER_WORDS: u32 = 8;

/** Bytes the descriptors of a ring of `capacity` messages take. */
export function ring_bytes(capacity: u32): u32 {
  return (RING_HEADER_WORDS + capacity * FRAME_WORDS) * 4;
}

/** Bytes the payloads of a ring of `capacity` slots of `slot_bytes` take. */
export function slots_bytes(capacity: u32, slot_bytes: u32): u32 {
  return capacity * slot_bytes;
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
  readonly slot_bytes: u32;
  private readonly mask: u32;
  /** The payloads, `capacity` of them, wherever in the memory they were put. */
  private readonly slots: U8Array;
  /** Where they begin, for the packet word to name one. */
  private readonly slot_base: u32;

  /** A ring already laid out: its descriptors at byte `at` of `mem`, its
   * payloads where its header says. */
  constructor(mem: ArrayBufferLike, at: u32) {
    const head = new Uint32Array(mem, at, RING_HEADER_WORDS);
    const capacity = head[0];
    if (capacity === 0 || (capacity & (capacity - 1)) !== 0) throw new Error(`ring: capacity ${capacity} at ${at} is not a power of two`);
    this.capacity = capacity;
    this.slot_bytes = head[3];
    this.slot_base = head[4];
    this.mask = capacity - 1;
    this.words = new Uint32Array(mem, at, RING_HEADER_WORDS + capacity * FRAME_WORDS);
    // The memory a ring lives in is never a SharedArrayBuffer here (the core
    // memory is fixed, not shared), and the packets it hands out are read by
    // the tree reader, which takes an ArrayBuffer view.
    this.slots = new Uint8Array(mem as ArrayBuffer, this.slot_base, slots_bytes(capacity, this.slot_bytes));
  }

  /** Lays out an empty ring: `capacity` descriptors at byte `at` of `mem`,
   * and as many payloads of `slot_bytes` at `slot_base`. */
  static init(mem: ArrayBufferLike, at: u32, capacity: u32, slot_base: u32, slot_bytes: u32): Ring {
    if (capacity === 0 || (capacity & (capacity - 1)) !== 0) throw new Error(`ring: capacity ${capacity} is not a power of two`);
    if (slot_bytes === 0 || (slot_bytes & (slot_bytes - 1)) !== 0 || slot_bytes % 8 !== 0) {
      throw new Error(`ring: a slot of ${slot_bytes} bytes is not a power of two of whole words`);
    }
    new Uint8Array(mem as ArrayBuffer, at, ring_bytes(capacity)).fill(0);
    const head = new Uint32Array(mem, at, RING_HEADER_WORDS);
    head[0] = capacity;
    head[3] = slot_bytes;
    head[4] = slot_base;
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
    return RING_HEADER_WORDS + ((position & this.mask) >>> 0) * FRAME_WORDS;
  }

  /** Where the payload of the message at `position` starts, the pair of
   * `entry_at`: one a slot, so it is free exactly when the slot is. */
  private slot_at(position: u32): u32 {
    return ((position & this.mask) >>> 0) * this.slot_bytes;
  }

  // ---- producer ----

  /**
   * Writes one message at the tail, its packet copied into the payload of the
   * slot it takes. EAGAIN, and nothing written, when the ring is full;
   * EMSGSIZE when the packet is larger than a slot.
   */
  push(tag: u32, cmd: u32, flags: u32 = 0, packet: U8Array | null = null, result: i32 = 0): Errno {
    if (this.len >= this.capacity) return Errno.EAGAIN;
    const w = this.words;
    let packet_at = 0;
    if (packet !== null) {
      const need = Math.ceil(packet.byteLength / 8) * 8;
      if (need > this.slot_bytes) return Errno.EMSGSIZE;
      const start = this.slot_at(w[2]);
      this.slots.set(packet, start);
      this.slots.fill(0, start + packet.byteLength, start + need);
      packet_at = this.slot_base + start;
    }
    const at = this.entry_at(w[2]);
    w[at + FRAME_TAG] = tag;
    w[at + FRAME_CMD] = cmd_word(cmd, flags);
    w[at + FRAME_RESULT] = result >>> 0;
    w[at + FRAME_PACKET] = packet_at;
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
    return this.words[at + FRAME_TAG];
  }

  cmd(at: u32): u32 {
    return cmd_of(this.words[at + FRAME_CMD]);
  }

  flags(at: u32): u32 {
    return flags_of(this.words[at + FRAME_CMD]);
  }

  result(at: u32): i32 {
    return this.words[at + FRAME_RESULT] | 0;
  }

  /** The message's packet, a view into its slot valid until `advance`, or
   * null when it has none. EBADMSG when it names bytes outside a slot. */
  packet(at: u32): U8Array | null {
    const packet_at = this.words[at + FRAME_PACKET];
    if (packet_at === 0) return null;
    const off = packet_at - this.slot_base;
    if (off < 0 || off % this.slot_bytes !== 0 || off + 16 > this.slots.byteLength || packet_bytes(this.slots, off) > this.slot_bytes) {
      throw new MarshalError(Errno.EBADMSG, `ring: message packet at ${packet_at} lies outside a slot`);
    }
    return this.slots.subarray(off, off + packet_bytes(this.slots, off));
  }

  /** Consumes the message at the head. */
  advance(): void {
    if (this.len === 0) throw new Error("ring: advance on an empty ring");
    this.words[1] = (this.words[1] + 1) >>> 0;
  }
}
