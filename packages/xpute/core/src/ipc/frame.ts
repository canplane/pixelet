// @xpute/core/ipc/frame.ts

/**
 * One command across a thread boundary, laid out in the buffer that carries
 * it.
 *
 * The buffer is the message: a request is written into it, the buffer is
 * transferred, the handler writes its reply into the same buffer, and it is
 * transferred back. Ownership moves with it — the sender's handle is
 * detached while it is away — so the two sides never share bytes and never
 * race, and nothing is copied except what the command itself chooses to
 * copy in and out.
 *
 * Layout, platform-endian (every host this runs on is little-endian, and a
 * WASM reader is): a fixed header of eight u32 words, then the command's
 * args, then its body at an 8-aligned offset. A body too large for the
 * buffer travels in a second buffer, the spill, and the header says so.
 *
 *   0  tx_id        worker id in the high bits, a sequence in the low
 *   1  cmd | flags  cmd in the low 16 bits, FrameFlag in the high 16
 *   2  errno        0, or a negative Errno
 *   3  args_bytes   args start at FRAME_HEADER_BYTES
 *   4  body_offset  8-aligned, or BODY_IN_SPILL
 *   5  body_bytes
 *   6  reserved
 *   7  reserved
 *
 * The shape is what April's frame had after its routing, selectors and
 * string commands were taken off (issue/20260913-boundaries-commands-and-
 * what-comes-back-from-april.md): a worker id in tx_id so a reply routes
 * without a lookup, `ACKREQ` as behaviour, an errno reply.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import type { Errno } from "@xpute/core/sys/errno.ts";

export const FRAME_HEADER_WORDS: u32 = 8;
export const FRAME_HEADER_BYTES: u32 = FRAME_HEADER_WORDS * 4;

/** body_offset when the body is in the spill buffer rather than the frame. */
export const BODY_IN_SPILL: u32 = 0xffffffff;

export const enum FrameFlag {
  /** A reply. Set by the runtime that sends it, never by a handler. */
  RES = 1,
  /** Exactly one reply is sent; without it, none is. */
  ACKREQ = 2,
}

export const WORKER_ID_SHIFT: u32 = 24;
const SEQ_MASK: u32 = (1 << WORKER_ID_SHIFT) - 1;

export function make_tx_id(worker_id: u32, seq: u32): u32 {
  return ((worker_id << WORKER_ID_SHIFT) | (seq & SEQ_MASK)) >>> 0;
}

export function tx_worker_id(tx_id: u32): u32 {
  return tx_id >>> WORKER_ID_SHIFT;
}

export function tx_seq(tx_id: u32): u32 {
  return tx_id & SEQ_MASK;
}

export function align8(n: u32): u32 {
  // In doubles: `&` would wrap a size past 2^31 negative, and a negative
  // body offset passes every "does it fit" check.
  return Math.ceil(n / 8) * 8;
}

/** The message a link carries: the frame, and the spill when there is one. */
export type FrameMessage = { frame: ArrayBuffer; spill: ArrayBuffer | null };

/**
 * A header read or written in place. One view per arrival: a transferred
 * buffer detaches every view the other side held, so there is nothing to
 * keep between messages.
 */
export class Frame {
  readonly buf: ArrayBuffer;
  private readonly words: Uint32Array;

  constructor(buf: ArrayBuffer) {
    if (buf.byteLength < FRAME_HEADER_BYTES) throw new Error(`ipc: frame of ${buf.byteLength} bytes is shorter than its header`);
    this.buf = buf;
    this.words = new Uint32Array(buf, 0, FRAME_HEADER_WORDS);
  }

  get tx_id(): u32 {
    return this.words[0];
  }
  set tx_id(v: u32) {
    this.words[0] = v;
  }

  get cmd(): u32 {
    return this.words[1] & 0xffff;
  }
  set cmd(v: u32) {
    this.words[1] = ((this.words[1] & 0xffff0000) | (v & 0xffff)) >>> 0;
  }

  get flags(): u32 {
    return this.words[1] >>> 16;
  }
  set flags(v: u32) {
    this.words[1] = (((v & 0xffff) << 16) | (this.words[1] & 0xffff)) >>> 0;
  }

  get errno(): Errno {
    return (this.words[2] | 0) as Errno;
  }
  set errno(v: Errno) {
    this.words[2] = v >>> 0;
  }

  get args_bytes(): u32 {
    return this.words[3];
  }
  get body_offset(): u32 {
    return this.words[4];
  }
  get body_bytes(): u32 {
    return this.words[5];
  }

  /** Starts a frame: header cleared, command and flags set, no args, no body. */
  begin(tx_id: u32, cmd: u32, flags: u32): void {
    this.words.fill(0);
    this.tx_id = tx_id;
    this.cmd = cmd;
    this.flags = flags;
  }

  /** Where the args live, for `args_bytes` bytes. */
  args_offset(): u32 {
    return FRAME_HEADER_BYTES;
  }

  /**
   * Declares the args and the body that follow them, and says where the
   * body goes: in the frame when it fits, otherwise in a spill of its own.
   * Returns the offset to write the body at, or BODY_IN_SPILL.
   */
  lay_out(args_bytes: u32, body_bytes: u32): u32 {
    const body_offset = align8(FRAME_HEADER_BYTES + args_bytes);
    if (body_offset > this.buf.byteLength) throw new Error(`ipc: args of ${args_bytes} bytes do not fit a ${this.buf.byteLength}-byte frame`);
    const in_frame = body_offset + body_bytes <= this.buf.byteLength;
    this.words[3] = args_bytes;
    this.words[4] = in_frame ? body_offset : BODY_IN_SPILL;
    this.words[5] = body_bytes;
    return this.words[4];
  }
}
