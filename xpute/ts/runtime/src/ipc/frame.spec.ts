// @xpute/runtime/ipc/frame.spec.ts
//
// GENERATED from spec/ipc/frame.json
//
// A frame is one message in a ring (ring.json), four 32-bit words; TAG, CMD, RESULT and
// PKT are their indices. The tag is the sender's id, and a reply carries the same tag, so
// the reply reaches the call that sent it. On an import the tag says what the import is
// about instead. The command shares its word with the flags. A reply carries a value or a
// negative errno. A packet, when the frame has one, lies in the ring's payload slot.
//
// All commands share one number space, `major << 8 | minor`. Majors 0x00 to 0x0F are
// xpute's own (abi/sys.json, abi/fs.json). Every
// other major is the program's to assign. The door applies xpute's own, and hands the program every other command. A runtime hands each frame to
// whatever answers its command's major.
//
// Flag bits 2 and up, and the result word of a submission, are reserved, so that a later
// use of them moves no other word.

import type { u32 } from "@xpute/kit/abi/word.ts";

/** The sender's id, which the reply carries back; on an import, what the import is about. */
export const TAG: u32 = 0 as number;

/** The command in the low FLAG_SHIFT bits, and the flags above them. */
export const CMD: u32 = 1 as number;

/** On a reply, a value or a negative errno; on any other frame, 0. */
export const RESULT: u32 = 2 as number;

/** Where the frame's XTP packet lies, as the transport names a place; 0 when it has none. */
export const PKT: u32 = 3 as number;

/** The frame's length in words. */
export const WORDS: u32 = 4 as number;

/** The first major a program's commands may take; the majors below it are xpute's to give. */
export const PROGRAM_MAJOR: u32 = 16 as number;

/** The bit at which the flags start in the CMD word. */
export const FLAG_SHIFT: u32 = 16 as number;

/** A flag that marks the frame as a reply; a frame the core posts without it is an import.
 * The ring code that posts the reply sets it, and a command's handler never does. */
export const RES: u32 = 1 as number;

/** A flag that asks for a reply in every case; without it, only a failure is replied to. */
export const ACKREQ: u32 = 2 as number;
