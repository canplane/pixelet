// @xpute/host/ipc/frame.spec.ts
//
// GENERATED from spec/ipc/frame.json
//
// A frame in a ring (ring.json): four words. The tag names the port at the guest's
// far side in its high bits — who sent a frame the guest takes, who is to have one it
// posts — and below them an id, so a reply, which keeps its submission's tag, goes back
// to its sender; the command shares its word with the flags; a reply carries a value or
// a negative errno; and a packet, where there is one, lies in the ring's payload slot.
//
// The host is port 0, and a command it sends is xpute's own (abi/cmd.json); every
// other number is the program's to give, and so is what its commands mean. The host
// hands a frame to the port it names where one is attached, and drops it where none
// is.
//
// Reserved for later, so taking them moves no word: flag bits 2 and up, and the result
// word on a submission.

import type { u32 } from "@xpute/kit/abi/word.ts";

/** The far port from PORT_SHIFT up; below it the sender's id, echoed on the
 * reply, or an event's subject. */
export const TAG: u32 = 0 as number;

/** The command in the low FLAG_SHIFT bits, the flags above them. */
export const CMD: u32 = 1 as number;

/** On a reply, a value or a negative errno; else 0. */
export const RESULT: u32 = 2 as number;

/** Where its XTP packet is, as the transport names it; 0 for none. */
export const PKT: u32 = 3 as number;

/** Words a frame takes. */
export const WORDS: u32 = 4 as number;

/** Where the far port starts in the TAG word: one of 256. */
export const PORT_SHIFT: u32 = 24 as number;

/** The host's port, whose commands are xpute's own. */
export const HOST: u32 = 0 as number;

/** Where the flags start in the CMD word. */
export const FLAG_SHIFT: u32 = 16 as number;

/** A flag: a reply, as against an event. Set by the sender, never by a handler. */
export const RES: u32 = 1 as number;

/** A flag: always reply; without it, only a failure is replied to. */
export const ACKREQ: u32 = 2 as number;
