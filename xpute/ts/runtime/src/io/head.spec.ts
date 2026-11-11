// @xpute/runtime/io/head.spec.ts
//
// GENERATED from spec/io/head.json
//
// The header at the front of a core's memory. The front is what the runtime lays out before
// the core's first turn: this header and the GPU stream (ARCHITECTURE.md). The header lies on
// the page past the core's stack. Both ends compute every number in the header from the same
// manifest, so the header records what the runtime laid; it does not tell the core where to
// look. On its first turn the core holds each word to its manifest and stops with ENOEXEC
// where one differs (xpute-core manifest.rs).

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as word from "./head/word.spec.ts";

/** The bytes "XPUT" read as a little-endian word. It tells a laid header from memory that
 * nothing has written. */
export const MAGIC: u32 = 1414877272 as number;

/** The index of each header word, counted from the header's start. */
export { word };

/** The header's length in words. */
export const WORDS: u32 = 3 as number;
