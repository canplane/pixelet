// @xpute/runtime/ipc/frame.ts

/**
 * A frame: one message, four words.
 *
 * Every boundary carries the same four things — who asked, what is asked,
 * what came back, and the packet that goes with it. Those are the format; a
 * transport says only where the words sit and how word 3 names the packet.
 * The ring (ipc/ring.ts) lays a frame out as a slot's descriptor and names
 * the packet by its address in the memory both sides see.
 *
 *   0  tag          the submitter's, echoed on the reply; on a signal, its
 *                   subject
 *   1  cmd | flags  cmd (cmd.ts: major << 8 | minor) in the low 16 bits,
 *                   FrameFlag in the high 16
 *   2  result       on a reply: a value, or a negative Errno; else 0
 *   3  packet       where the message's XTP packet is, as the transport
 *                   names it, or 0 for none — its length is in its header
 *
 * ## Reserved
 *
 * Reserved here is not spare room: each is spoken for, and kept free so
 * that taking it moves no word already on the wire
 * (issue/20260913-boundaries-commands-and-what-comes-back-from-april.md).
 *
 * **`tag`'s high bits — routing.** One ring joins two sides, so the whole
 * word is the submitter's and a reply needs no address. Put a switch between
 * them — several workers, or a server — and a reply has to find its way back
 * without a lookup, which is what a sender id in the high bits buys. Take
 * them from the top; a sequence keeps the low bits it has.
 *
 * **`FrameFlag`'s bits 2 and up — what the low half of word 1 means.** The
 * low half is a command. A signal dispatched by vector differs from one only
 * in how its number is read, so it takes a flag bit rather than a second
 * word: set, the low half is a vector; clear, a command.
 *
 * **Word 2 while `RES` is clear.** A submission has nothing to report, so
 * the word is 0 and is free to mean something on the way out.
 */

import type { u32 } from "@xpute/core/abi/word.ts";

/** Words a frame takes. */
export const FRAME_WORDS: u32 = 4;

export const FRAME_TAG: u32 = 0;
export const FRAME_CMD: u32 = 1;
export const FRAME_RESULT: u32 = 2;
export const FRAME_PACKET: u32 = 3;

/** Where the flags start in word 1. */
export const FLAG_SHIFT: u32 = 16;
/** The low half of word 1. */
export const CMD_MASK: u32 = (1 << FLAG_SHIFT) - 1;

/** What a frame asks for, or reports. Bits 2 and up are reserved above. */
export const enum FrameFlag {
  /**
   * A reply. Set by the side that sends it, never by a handler: on a
   * completion it tells a reply from a signal the other side raised.
   */
  RES = 1,
  /** Exactly one reply is sent; without it, none is. */
  ACKREQ = 2,
}

/** Word 1 from a command and its flags. */
export function cmd_word(cmd: u32, flags: u32): u32 {
  return ((flags << FLAG_SHIFT) | (cmd & CMD_MASK)) >>> 0;
}

/** The command word 1 carries. */
export function cmd_of(word: u32): u32 {
  return word & CMD_MASK;
}

/** The flags word 1 carries. */
export function flags_of(word: u32): u32 {
  return word >>> FLAG_SHIFT;
}
