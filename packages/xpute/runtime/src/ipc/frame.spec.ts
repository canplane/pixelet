// @xpute/runtime/ipc/frame.spec.ts
//
// GENERATED from spec/xpute/ipc/frame.json — do not edit.
//
// A message in a ring (ring.json): four words, a frame. The submitter's tag comes back on
// the reply; the command shares its word with the flags; a reply carries a value or a
// negative errno; and a packet, where there is one, lies in the ring's payload slot.
//
// Reserved for later, so taking them moves no word: the tag's high bits (a sender id for
// routing), flag bits 2 and up, and the result word on a submission.

import type { u32 } from "@xpute/core/abi/word.ts";

/** The submitter's, echoed on the reply; a signal's subject. */
export const TAG: u32 = 0 as number;

/** The command in the low FLAG_SHIFT bits, the flags above them. */
export const CMD: u32 = 1 as number;

/** On a reply, a value or a negative errno; else 0. */
export const RESULT: u32 = 2 as number;

/** Where its XTP packet is, as the transport names it; 0 for none. */
export const PACKET: u32 = 3 as number;

/** Words a frame takes. */
export const WORDS: u32 = 4 as number;

/** Where the flags start in the CMD word. */
export const FLAG_SHIFT: u32 = 16 as number;

/** A flag: a reply, as against a signal. Set by the sender, never by a handler. */
export const RES: u32 = 1 as number;

/** A flag: always reply; without it, only a failure is replied to. */
export const ACKREQ: u32 = 2 as number;
