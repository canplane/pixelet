// @xpute/runtime/ipc/head.spec.ts
//
// GENERATED from spec/ipc/head.json
//
// The header at the front of a core's memory. The front is what the runtime lays out before
// the core's first turn: this header, the two rings and their slots, and the GPU stream
// (ARCHITECTURE.md). The header lies on the page past the core's stack. The runtime writes it
// as io_uring's setup lays its rings, and lays each ring's own header too (ring.json), with
// its positions at 0. Both ends compute every number in the header from the same machine, so
// the header records what the runtime laid; it does not tell the core where to look. On its
// first turn the core holds each word to its machine and stops with ENOEXEC where one
// differs (xpute-core machine.rs).

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as word from "./head/word.spec.ts";

/** The bytes "XPUT" read as a little-endian word. It tells a laid header from memory that
 * nothing has written. */
export const MAGIC: u32 = 1414877272 as number;

/** The index of each header word, counted from the header's start. */
export { word };

/** The header's length in words. */
export const WORDS: u32 = 7 as number;
