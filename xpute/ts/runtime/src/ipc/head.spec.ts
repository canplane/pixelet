// @xpute/runtime/ipc/head.spec.ts
//
// GENERATED from spec/ipc/head.json
//
// The front's header: the words a runtime writes on the page past a core's stack before its
// first turn, as io_uring's setup lays its rings, and that the core's boot holds to its
// machine, stopping with ENOEXEC where one differs (xpute-core machine.rs). Both ends
// compute every number in it from the same machine, so it names what the runtime laid, not
// where to look: the rings' sizes and places, and the GPU stream's. The runtime lays each
// ring's own header too (ring.json), its positions at 0.

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as word from "./head/word.spec.ts";

/** "XPUT" as four bytes read little-endian: what tells a header from memory nothing wrote. */
export const MAGIC: u32 = 1414877272 as number;

/** Raised when a word comes to mean something else. 1 was the toc's, a directory of keys. */
export const VERSION: u32 = 2 as number;

/** Where each of the header's words is, from its start. */
export { word };

/** Words the header takes. */
export const WORDS: u32 = 8 as number;
