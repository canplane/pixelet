// @xpute/runtime/ipc/ring.ts

/**
 * A single-producer, single-consumer ring of frames (frame.ts) in memory both
 * sides see. Descriptors and payloads lie apart; message `n` takes slot
 * `n & (capacity - 1)` in both, so a payload is free exactly when its slot is.
 * Each side writes only its own position word, so no lock is needed.
 *
 * Header words: 0 capacity (power of two), 1 head, 2 tail, 3 slot_bytes (power
 * of two, multiple of 8), 4 slot_base, 5..7 reserved.
 */

import type { i32, u32 } from "@xpute/core/abi/word.ts";
import type { U8Array } from "@xpute/core/abi/array.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { MarshalError } from "@xpute/core/status/error.ts";

import { cmd_of, cmd_word, flags_of, FRAME_CMD, FRAME_PACKET, FRAME_RESULT, FRAME_TAG, FRAME_WORDS } from "@xpute/runtime/ipc/frame.ts";
import { InvariantError } from "@xpute/core/status/error.ts";

export const RING_HEADER_WORDS: u32 = 8;

export function ring_bytes(capacity: u32): u32 {
  return (RING_HEADER_WORDS + capacity * FRAME_WORDS) * 4;
}

export function slots_bytes(capacity: u32, slot_bytes: u32): u32 {
  return capacity * slot_bytes;
}

/** A packet's 16-byte header plus the payload its second word names, rounded to a word. */
function packet_bytes(u8: U8Array, at: u32): u32 {
  const payload = u8[at + 8] | (u8[at + 9] << 8) | (u8[at + 10] << 16) | (u8[at + 11] << 24);
  return Math.ceil((16 + (payload >>> 0)) / 8) * 8;
}

export class Ring {
  readonly words: Uint32Array;
  readonly capacity: u32;
  readonly slot_bytes: u32;
  private readonly mask: u32;
  private readonly slots: U8Array;
  private readonly slot_base: u32;

  constructor(mem: ArrayBufferLike, at: u32) {
    const head = new Uint32Array(mem, at, RING_HEADER_WORDS);
    const capacity = head[0];
    if (capacity === 0 || (capacity & (capacity - 1)) !== 0) throw new InvariantError(Errno.EINVAL);
    this.capacity = capacity;
    this.slot_bytes = head[3];
    this.slot_base = head[4];
    this.mask = capacity - 1;
    this.words = new Uint32Array(mem, at, RING_HEADER_WORDS + capacity * FRAME_WORDS);
    // Never a SharedArrayBuffer here, and the tree reader takes an ArrayBuffer view.
    this.slots = new Uint8Array(mem as ArrayBuffer, this.slot_base, slots_bytes(capacity, this.slot_bytes));
  }

  static init(mem: ArrayBufferLike, at: u32, capacity: u32, slot_base: u32, slot_bytes: u32): Ring {
    if (capacity === 0 || (capacity & (capacity - 1)) !== 0) throw new InvariantError(Errno.EINVAL);
    if (slot_bytes === 0 || (slot_bytes & (slot_bytes - 1)) !== 0 || slot_bytes % 8 !== 0) {
      throw new InvariantError(Errno.EINVAL);
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

  get len(): u32 {
    return (this.words[2] - this.words[1]) >>> 0;
  }

  entry_at(position: u32): u32 {
    return RING_HEADER_WORDS + ((position & this.mask) >>> 0) * FRAME_WORDS;
  }

  private slot_at(position: u32): u32 {
    return ((position & this.mask) >>> 0) * this.slot_bytes;
  }

  /** EAGAIN, writing nothing, when full; EMSGSIZE when the packet exceeds a slot. */
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

  /** -1 when empty. */
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

  /** A view valid until `advance`; EBADMSG when it names bytes outside a slot. */
  packet(at: u32): U8Array | null {
    const packet_at = this.words[at + FRAME_PACKET];
    if (packet_at === 0) return null;
    const off = packet_at - this.slot_base;
    if (off < 0 || off % this.slot_bytes !== 0 || off + 16 > this.slots.byteLength || packet_bytes(this.slots, off) > this.slot_bytes) {
      throw new MarshalError(Errno.EBADMSG);
    }
    return this.slots.subarray(off, off + packet_bytes(this.slots, off));
  }

  advance(): void {
    if (this.len === 0) throw new InvariantError(Errno.ENOTRECOVERABLE);
    this.words[1] = (this.words[1] + 1) >>> 0;
  }
}
