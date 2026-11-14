// @xpute/link/frame.ts

/**
 * A frame's words are laid out by link/spec/frame.json: the request's tag, its
 * command, the result, and the packet's length. The command is the RES bit,
 * the handle the callee issued and the method's place among its object's.
 * The packet, the method's args or its answer's value in the Canonical ABI's
 * layout (cabi.ts), lies right past the words. The words
 * are little-endian (xpute-link frame.rs).
 */

import type { i32, u32 } from "@xpute/kit/abi/word.ts";

import { CMD, METHOD_BITS, PKT, RES, RESULT, TAG, WORDS } from "./frame.spec.ts";

/** The bytes a frame's words take, before its packet. */
export const WORD_BYTES: u32 = WORDS * 4;

export interface Frame {
  tag: u32;
  /** A later answer, to the receiver's request `tag`. */
  res: boolean;
  handle: u32;
  method: u32;
  result: i32;
  pkt_len: u32;
}

/** The frame laid at `at`. */
export function read(v: DataView, at: u32): Frame {
  const word = (k: u32): u32 => v.getUint32(at + 4 * k, true);
  const cmd = word(CMD);
  return {
    tag: word(TAG),
    res: (cmd & RES) !== 0,
    handle: (cmd & ~RES) >>> METHOD_BITS,
    method: cmd & ((1 << METHOD_BITS) - 1),
    result: word(RESULT) | 0,
    pkt_len: word(PKT),
  };
}

/** Lays the frame's words at `at`. */
export function lay(v: DataView, at: u32, f: Frame): void {
  v.setUint32(at + 4 * TAG, f.tag, true);
  v.setUint32(at + 4 * CMD, ((f.res ? RES : 0) | (f.handle << METHOD_BITS) | f.method) >>> 0, true);
  v.setUint32(at + 4 * RESULT, f.result >>> 0, true);
  v.setUint32(at + 4 * PKT, f.pkt_len, true);
}
