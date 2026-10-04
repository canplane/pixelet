// @xpute/runtime/ipc/ring.ts

/** A ring of frames, laid out by spec/xpute/ipc/ring.json. */

import type { i32, u32 } from "@xpute/core/abi/word.ts";
import type { U8Array } from "@xpute/core/abi/array.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { InvariantError, MarshalError } from "@xpute/core/status/error.ts";

import * as frame from "./frame.ts";
import { CAPACITY, HEAD, HEADER_WORDS, SLOT_BASE, SLOT_BYTES, TAIL } from "./ring.spec.ts";

export function ring_bytes(capacity: u32): u32 {
  return (HEADER_WORDS + capacity * frame.WORDS) * 4;
}

export function slots_bytes(capacity: u32, slot_bytes: u32): u32 {
  return capacity * slot_bytes;
}

/** Payloads are padded to 8 bytes. */
function padded(bytes: u32): u32 {
  return Math.ceil(bytes / 8) * 8;
}

/** A power of two, so a position masks to its slot. */
function isPowerOfTwo(n: u32): boolean {
  return n !== 0 && (n & (n - 1)) === 0;
}

/** A packet's 16-byte header plus the payload its second word names. */
function packet_bytes(u8: U8Array, at: u32): u32 {
  const payload = u8[at + 8] | (u8[at + 9] << 8) | (u8[at + 10] << 16) | (u8[at + 11] << 24);
  return padded(16 + (payload >>> 0));
}

export class Ring {
  readonly words: Uint32Array;
  readonly capacity: u32;
  readonly slot_bytes: u32;
  private readonly mask: u32;
  private readonly slots: U8Array;
  private readonly slot_base: u32;

  constructor(mem: ArrayBufferLike, at: u32) {
    const head = new Uint32Array(mem, at, HEADER_WORDS);
    const capacity = head[CAPACITY];
    if (!isPowerOfTwo(capacity)) throw new InvariantError(Errno.EINVAL);
    this.capacity = capacity;
    this.slot_bytes = head[SLOT_BYTES];
    this.slot_base = head[SLOT_BASE];
    this.mask = capacity - 1;
    this.words = new Uint32Array(mem, at, ring_bytes(capacity) / 4);
    // Never a SharedArrayBuffer here, and the tree reader takes an ArrayBuffer view.
    this.slots = new Uint8Array(mem as ArrayBuffer, this.slot_base, slots_bytes(capacity, this.slot_bytes));
  }

  static init(mem: ArrayBufferLike, at: u32, capacity: u32, slot_base: u32, slot_bytes: u32): Ring {
    if (!isPowerOfTwo(capacity)) throw new InvariantError(Errno.EINVAL);
    if (!isPowerOfTwo(slot_bytes) || slot_bytes % 8 !== 0) throw new InvariantError(Errno.EINVAL);
    new Uint8Array(mem as ArrayBuffer, at, ring_bytes(capacity)).fill(0);
    const head = new Uint32Array(mem, at, HEADER_WORDS);
    head[CAPACITY] = capacity;
    head[SLOT_BYTES] = slot_bytes;
    head[SLOT_BASE] = slot_base;
    return new Ring(mem, at);
  }

  get head(): u32 {
    return this.words[HEAD];
  }

  get tail(): u32 {
    return this.words[TAIL];
  }

  get len(): u32 {
    return (this.tail - this.head) >>> 0;
  }

  entry_at(position: u32): u32 {
    return HEADER_WORDS + ((position & this.mask) >>> 0) * frame.WORDS;
  }

  private slot_at(position: u32): u32 {
    return ((position & this.mask) >>> 0) * this.slot_bytes;
  }

  /** EAGAIN, writing nothing, when full; EMSGSIZE when the packet exceeds a slot. */
  push(tag: u32, cmd: u32, flags: u32 = 0, packet: U8Array | null = null, result: i32 = 0): Errno {
    if (this.len >= this.capacity) return Errno.EAGAIN;
    let packet_at = 0;
    if (packet !== null) {
      const need = padded(packet.byteLength);
      if (need > this.slot_bytes) return Errno.EMSGSIZE;
      const start = this.slot_at(this.tail);
      this.slots.set(packet, start);
      this.slots.fill(0, start + packet.byteLength, start + need);
      packet_at = this.slot_base + start;
    }
    const w = this.words;
    const at = this.entry_at(this.tail);
    w[at + frame.TAG] = tag;
    w[at + frame.CMD] = frame.cmd_word(cmd, flags);
    w[at + frame.RESULT] = result >>> 0;
    w[at + frame.PACKET] = packet_at;
    w[TAIL] = (w[TAIL] + 1) >>> 0;
    return Errno.OK;
  }

  /** -1 when empty. */
  peek(): number {
    return this.len === 0 ? -1 : this.entry_at(this.head);
  }

  tag(at: u32): u32 {
    return this.words[at + frame.TAG];
  }

  cmd(at: u32): u32 {
    return frame.cmd_of(this.words[at + frame.CMD]);
  }

  flags(at: u32): u32 {
    return frame.flags_of(this.words[at + frame.CMD]);
  }

  result(at: u32): i32 {
    return this.words[at + frame.RESULT] | 0;
  }

  /** A view valid until `advance`; EBADMSG when it names bytes outside a slot. */
  packet(at: u32): U8Array | null {
    const packet_at = this.words[at + frame.PACKET];
    if (packet_at === 0) return null;
    const off = packet_at - this.slot_base;
    if (off < 0 || off % this.slot_bytes !== 0 || off + 16 > this.slots.byteLength) throw new MarshalError(Errno.EBADMSG);
    const len = packet_bytes(this.slots, off);
    if (len > this.slot_bytes) throw new MarshalError(Errno.EBADMSG);
    return this.slots.subarray(off, off + len);
  }

  advance(): void {
    if (this.len === 0) throw new InvariantError(Errno.ENOTRECOVERABLE);
    this.words[HEAD] = (this.head + 1) >>> 0;
  }
}
