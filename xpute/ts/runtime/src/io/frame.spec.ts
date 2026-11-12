// @xpute/runtime/io/frame.spec.ts
//
// GENERATED from spec/io/frame.json
//
// A frame is one method called across the link, four 32-bit words; TAG, CMD, RESULT and
// PKT are their indices, little-endian. The tag is the request, unique among the sender's
// pending ones, and a later answer carries it back. The command is the RES bit, the handle
// the callee issued above METHOD_BITS, and the method's place among its object's below
// them. An answer carries a value or a negative errno. The method's args, an XTP packet,
// lie right past the words, and PKT says how long they are (wit/link.wit).

import type { u32 } from "@xpute/kit/abi/word.ts";

/** The request, which a later answer carries back. */
export const TAG: u32 = 0 as number;

/** RES, the handle and the method. */
export const CMD: u32 = 1 as number;

/** On an answer, a value or a negative errno; on a request, 0. */
export const RESULT: u32 = 2 as number;

/** The length in bytes of the frame's packet, which lies right past its words; 0 when it has none. */
export const PKT: u32 = 3 as number;

/** The frame's length in words. */
export const WORDS: u32 = 4 as number;

/** The bit of the command that marks a later answer. */
export const RES: u32 = 2147483648 as number;

/** The bits of the command below the handle, which hold the method. */
export const METHOD_BITS: u32 = 8 as number;
