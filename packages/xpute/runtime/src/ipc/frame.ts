// @xpute/runtime/ipc/frame.ts

/**
 * One message, four words:
 *   0  tag          the submitter's, echoed on the reply; a signal's subject
 *   1  cmd | flags  cmd (major << 8 | minor) low 16 bits, FrameFlag high 16
 *   2  result       on a reply a value or a negative Errno, else 0
 *   3  packet       where the XTP packet is, as the transport names it, or 0
 * tag's high bits and FrameFlag bits 2 and up are reserved for routing and a
 * vector-dispatched signal, so taking them moves no word already on the wire.
 */

import type { u32 } from "@xpute/core/abi/word.ts";

export const FRAME_WORDS: u32 = 4;

export const FRAME_TAG: u32 = 0;
export const FRAME_CMD: u32 = 1;
export const FRAME_RESULT: u32 = 2;
export const FRAME_PACKET: u32 = 3;

const FLAG_SHIFT: u32 = 16;
const CMD_MASK: u32 = (1 << FLAG_SHIFT) - 1;

/** Bits 2 and up are reserved. */
export const enum FrameFlag {
  /** Set by the sending side, never a handler: it tells a reply from a signal. */
  RES = 1,
  /** Exactly one reply; without it, one only on failure. */
  ACKREQ = 2,
}

export function cmd_word(cmd: u32, flags: u32): u32 {
  return ((flags << FLAG_SHIFT) | (cmd & CMD_MASK)) >>> 0;
}

export function cmd_of(word: u32): u32 {
  return word & CMD_MASK;
}

export function flags_of(word: u32): u32 {
  return word >>> FLAG_SHIFT;
}
