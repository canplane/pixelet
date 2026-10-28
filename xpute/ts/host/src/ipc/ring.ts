// @xpute/host/ipc/ring.ts

/** A ring of frames, laid out by spec/ipc/ring.json. */

import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import type { U8Array } from "@xpute/kit/abi/array.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { InvariantError, MarshalError } from "@xpute/kit/status/error.ts";
import type { PacketWriter } from "@xpute/kit/wire/xtp/mod.ts";
import { HDR_SZ, WORD_SZ } from "@xpute/kit/wire/xtp/spec.ts";

import * as frame from "./frame.ts";
import {
  CAP,
  HEAD,
  HEADER_WORDS,
  SLOT_BASE,
  SLOT_BYTES,
  TAIL,
} from "./ring.spec.ts";

export function ring_bytes(cap: u32): u32 {
  return (HEADER_WORDS + cap * frame.WORDS) * 4;
}

export function slots_bytes(cap: u32, slot_bytes: u32): u32 {
  return cap * slot_bytes;
}

/** Payloads are padded to XTP's word (`WORD_SZ`). */
function padded(bytes: u32): u32 {
  return Math.ceil(bytes / WORD_SZ) * WORD_SZ;
}

/** A power of two, so a position masks to its slot. */
function is_pow2(n: u32): boolean {
  return n !== 0 && (n & (n - 1)) === 0;
}

/** A packet's header plus the payload its second word names. */
function packet_bytes(u8: U8Array, at: u32): u32 {
  const word = at + WORD_SZ;
  const payload = u8[word] | (u8[word + 1] << 8) | (u8[word + 2] << 16) |
    (u8[word + 3] << 24);
  return padded(HDR_SZ + (payload >>> 0));
}

export class Ring {
  readonly words: Uint32Array;
  readonly cap: u32;
  readonly slot_bytes: u32;
  private readonly mask: u32;
  private readonly slots: U8Array;
  private readonly slot_view: DataView;
  private readonly slot_base: u32;

  constructor(mem: ArrayBufferLike, at: u32) {
    const head = new Uint32Array(mem, at, HEADER_WORDS);
    const cap = head[CAP];
    if (!is_pow2(cap)) throw new InvariantError(Errno.EINVAL);
    this.cap = cap;
    this.slot_bytes = head[SLOT_BYTES];
    this.slot_base = head[SLOT_BASE];
    this.mask = cap - 1;
    this.words = new Uint32Array(mem, at, ring_bytes(cap) / 4);
    // Never a SharedArrayBuffer here, and the tree reader takes an ArrayBuffer view.
    this.slots = new Uint8Array(
      mem as ArrayBuffer,
      this.slot_base,
      slots_bytes(cap, this.slot_bytes),
    );
    this.slot_view = new DataView(
      mem as ArrayBuffer,
      this.slot_base,
      slots_bytes(cap, this.slot_bytes),
    );
  }

  static init(
    mem: ArrayBufferLike,
    at: u32,
    cap: u32,
    slot_base: u32,
    slot_bytes: u32,
  ): Ring {
    if (!is_pow2(cap)) throw new InvariantError(Errno.EINVAL);
    if (!is_pow2(slot_bytes) || slot_bytes % WORD_SZ !== 0) {
      throw new InvariantError(Errno.EINVAL);
    }
    new Uint8Array(mem as ArrayBuffer, at, ring_bytes(cap)).fill(0);
    const head = new Uint32Array(mem, at, HEADER_WORDS);
    head[CAP] = cap;
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

  /** Begins `w` over the tail's slot, a packet of `count` children written
   * in place; false, beginning nothing, when the ring is full. */
  write(w: PacketWriter, count: u32): boolean {
    if (this.len >= this.cap) return false;
    w.begin(
      this.slots,
      this.slot_view,
      this.slot_at(this.tail),
      this.slot_bytes,
      count,
    );
    return true;
  }

  /** EAGAIN, writing nothing, when full; EMSGSIZE when the packet a writer wrote
   * in the tail's slot (`write`) exceeds it: refused, never split. */
  push(
    tag: u32,
    cmd: u32,
    flags: u32 = 0,
    packet: PacketWriter | null = null,
    result: i32 = 0,
  ): Errno {
    if (this.len >= this.cap) return Errno.EAGAIN;
    let packet_at = 0;
    if (packet !== null) {
      const start = this.slot_at(this.tail);
      if (packet.start !== start) {
        throw new InvariantError(Errno.ENOTRECOVERABLE);
      }
      if (packet.finish() < 0) return Errno.EMSGSIZE;
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
    if (
      off < 0 || off % this.slot_bytes !== 0 ||
      off + HDR_SZ > this.slots.byteLength
    ) throw new MarshalError(Errno.EBADMSG);
    const len = packet_bytes(this.slots, off);
    if (len > this.slot_bytes) throw new MarshalError(Errno.EBADMSG);
    return this.slots.subarray(off, off + len);
  }

  advance(): void {
    if (this.len === 0) throw new InvariantError(Errno.ENOTRECOVERABLE);
    this.words[HEAD] = (this.head + 1) >>> 0;
  }
}
