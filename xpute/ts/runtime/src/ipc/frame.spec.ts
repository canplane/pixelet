// @xpute/runtime/ipc/frame.spec.ts
//
// GENERATED from spec/ipc/frame.json
//
// A frame in a ring (ring.json): four words. The tag is the sender's id, which a reply
// keeps, so it goes back to the call that sent it, or an import's subject; the command
// shares its word with the flags; a reply carries a value or a negative errno; and a
// packet, where there is one, lies in the ring's payload slot.
//
// Commands share one number space, `major << 8 | minor`: majors 0x00 to 0x0F are
// xpute's own (abi/cmd.json, abi/fs.json), and every other is the program's to give,
// and so is what its commands mean. A runtime hands a frame to what answers its command's
// major.
//
// Reserved for later, so taking them moves no word: flag bits 2 and up, and the result
// word on a submission.

import type { u32 } from "@xpute/kit/abi/word.ts";

/** The sender's id, echoed on the reply, or an import's subject. */
export const TAG: u32 = 0 as number;

/** The command in the low FLAG_SHIFT bits, the flags above them. */
export const CMD: u32 = 1 as number;

/** On a reply, a value or a negative errno; else 0. */
export const RESULT: u32 = 2 as number;

/** Where its XTP packet is, as the transport names it; 0 for none. */
export const PKT: u32 = 3 as number;

/** Words a frame takes. */
export const WORDS: u32 = 4 as number;

/** The first major a program's commands take; those below it are xpute's own. */
export const PROGRAM_MAJOR: u32 = 16 as number;

/** Where the flags start in the CMD word. */
export const FLAG_SHIFT: u32 = 16 as number;

/** A flag: a reply's frame, as against an import's. Set by the sender, never by a handler. */
export const RES: u32 = 1 as number;

/** A flag: always reply; without it, only a failure is replied to. */
export const ACKREQ: u32 = 2 as number;
