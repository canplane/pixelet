// @xpute/link/frame.ts

/**
 * A frame's words are laid out by `link/spec/frame.json5`: the request's tag,
 * its command, the result, and the packet's length. The command is the `RES`
 * bit, the handle the callee issued and the method's place among its
 * object's. The packet, the method's args or its answer's value in the
 * Canonical ABI's layout (`cabi.ts`), lies right past the words. The words
 * are little-endian (xpute-link `frame.rs`).
 */

import type { i32, u32 } from "@xpute/kit/abi/word.ts";

import {
  CMD,
  METHOD_BITS,
  PKT,
  RES,
  RESULT,
  TAG,
  WORDS,
} from "./frame.spec.ts";

/** The size of a frame's words, its header before its packet. */
export const HDR_SZ: u32 = WORDS * 4;

export interface Frame {
  tag: u32;
  /** An answer, to the receiver's request `tag`. */
  res: boolean;
  handle: u32;
  method: u32;
  result: i32;
  pkt_len: u32;
}

/** A frame of zero words, for `read` to fill. */
export function frame(): Frame {
  return { tag: 0, res: false, handle: 0, method: 0, result: 0, pkt_len: 0 };
}

/** Reads the frame laid at `at` into `f`, and answers it. A side reads each
 * frame into one it keeps, so a crossing makes no object. */
export function read(v: DataView<ArrayBufferLike>, at: u32, f: Frame): Frame {
  const cmd = v.getUint32(at + 4 * CMD, true);
  f.tag = v.getUint32(at + 4 * TAG, true);
  f.res = (cmd & RES) !== 0;
  f.handle = (cmd & ~RES) >>> METHOD_BITS;
  f.method = cmd & ((1 << METHOD_BITS) - 1);
  f.result = v.getUint32(at + 4 * RESULT, true) | 0;
  f.pkt_len = v.getUint32(at + 4 * PKT, true);
  return f;
}

/** Lays the frame's words at `at`. */
export function lay(v: DataView<ArrayBufferLike>, at: u32, f: Frame): void {
  v.setUint32(at + 4 * TAG, f.tag, true);
  v.setUint32(
    at + 4 * CMD,
    ((f.res ? RES : 0) | (f.handle << METHOD_BITS) | f.method) >>> 0,
    true,
  );
  v.setUint32(at + 4 * RESULT, f.result >>> 0, true);
  v.setUint32(at + 4 * PKT, f.pkt_len, true);
}
