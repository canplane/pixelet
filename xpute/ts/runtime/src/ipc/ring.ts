// @xpute/runtime/ipc/ring.ts

/** A ring of frames, laid out by spec/ipc/ring.json. Each end writes one of
 * the two positions: the producer writes TAIL and the consumer writes HEAD.
 * An end reads its own position plainly, stores it with release after it is
 * done with the frames it covers, and loads the other end's with acquire
 * before it touches those frames (atomic.ts), so that a core on another
 * thread sees the frames the positions cover. */

import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import type { U8Array } from "@xpute/kit/abi/array.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { InvariantError, MarshalError } from "@xpute/kit/status/error.ts";
import type { PacketWriter } from "@xpute/kit/wire/xtp/mod.ts";
import { HDR_SZ, WORD_SZ } from "@xpute/kit/wire/xtp/spec.ts";

import { load_acquire, store_release } from "./atomic";
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

function padded(bytes: u32): u32 {
  return Math.ceil(bytes / WORD_SZ) * WORD_SZ;
}

/** A capacity must be a power of two, so that a position masks to its
 * slot. */
function is_pow2(n: u32): boolean {
  return n !== 0 && (n & (n - 1)) === 0;
}

/** A packet takes its header plus the payload length its second word holds,
 * padded. */
function pkt_bytes(u8: U8Array, at: u32): u32 {
  const word = at + WORD_SZ;
  const payload =
    u8[word] |
    (u8[word + 1] << 8) |
    (u8[word + 2] << 16) |
    (u8[word + 3] << 24);
  return padded(HDR_SZ + (payload >>> 0));
}

export class Ring {
  /** This is where the ring's words begin in the memory. */
  readonly at: u32;
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
    this.at = at;
    this.cap = cap;
    this.slot_bytes = head[SLOT_BYTES];
    this.slot_base = head[SLOT_BASE];
    this.mask = cap - 1;
    this.words = new Uint32Array(mem, at, ring_bytes(cap) / 4);
    // The memory is never a SharedArrayBuffer here, and the tree reader takes
    // a view of an ArrayBuffer.
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
    return load_acquire(this.words, HEAD);
  }

  get tail(): u32 {
    return load_acquire(this.words, TAIL);
  }

  /** How many frames are not yet taken, counted from the producer's side:
   * TAIL is read plainly, as only its writer may. */
  get len(): u32 {
    return (this.words[TAIL] - this.head) >>> 0;
  }

  entry_at(pos: u32): u32 {
    return HEADER_WORDS + ((pos & this.mask) >>> 0) * frame.WORDS;
  }

  private slot_at(pos: u32): u32 {
    return ((pos & this.mask) >>> 0) * this.slot_bytes;
  }

  /** Begins `w` over the tail's slot, for a packet of `cnt` children written
   * in place. Returns false, and begins nothing, when the ring is full. */
  write(w: PacketWriter, cnt: u32): boolean {
    if (this.len >= this.cap) return false;
    w.begin(
      this.slots,
      this.slot_view,
      this.slot_at(this.words[TAIL]),
      this.slot_bytes,
      cnt,
    );
    return true;
  }

  /** Publishes a frame at the tail. Returns EAGAIN, writing nothing, when the
   * ring is full, and EMSGSIZE when the packet written in the tail's slot
   * (`write`) does not fit it; a packet is refused, never split. */
  push(
    tag: u32,
    cmd: u32,
    flags: u32 = 0,
    pkt: PacketWriter | null = null,
    result: i32 = 0,
  ): i32 {
    if (this.len >= this.cap) return Errno.EAGAIN;
    const w = this.words;
    const tail = w[TAIL];
    let pkt_at = 0;
    if (pkt !== null) {
      const start = this.slot_at(tail);
      if (pkt.start !== start) {
        throw new InvariantError(Errno.ENOTRECOVERABLE);
      }
      if (pkt.finish() < 0) return Errno.EMSGSIZE;
      pkt_at = this.slot_base + start;
    }
    const at = this.entry_at(tail);
    w[at + frame.TAG] = tag;
    w[at + frame.CMD] = frame.cmd_word(cmd, flags);
    w[at + frame.RESULT] = result >>> 0;
    w[at + frame.PKT] = pkt_at;
    store_release(w, TAIL, (tail + 1) >>> 0);
    return Errno.OK;
  }

  /** The entry that begins at byte `off` of the memory, in the form `peek`
   * returns. Throws EBADMSG where no entry begins there. */
  entry_of(off: u32): u32 {
    const word = (off - this.at) / 4;
    if (
      !Number.isInteger(word) ||
      word < HEADER_WORDS ||
      (word - HEADER_WORDS) % frame.WORDS !== 0 ||
      word >= this.words.length
    )
      throw new MarshalError(Errno.EBADMSG);
    return word;
  }

  /** The entry at the head, or -1 when the ring is empty. */
  peek(): number {
    const head = this.words[HEAD];
    return this.tail === head ? -1 : this.entry_at(head);
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

  /** The entry's packet, as a view valid until `advance`, or null for none.
   * Throws EBADMSG when the frame names bytes outside a slot. */
  pkt(at: u32): U8Array | null {
    const pkt_at = this.words[at + frame.PKT];
    if (pkt_at === 0) return null;
    const off = pkt_at - this.slot_base;
    if (
      off < 0 ||
      off % this.slot_bytes !== 0 ||
      off + HDR_SZ > this.slots.byteLength
    )
      throw new MarshalError(Errno.EBADMSG);
    const len = pkt_bytes(this.slots, off);
    if (len > this.slot_bytes) throw new MarshalError(Errno.EBADMSG);
    return this.slots.subarray(off, off + len);
  }

  /** Moves the head past the entry `peek` gave. */
  advance(): void {
    store_release(this.words, HEAD, (this.words[HEAD] + 1) >>> 0);
  }
}
