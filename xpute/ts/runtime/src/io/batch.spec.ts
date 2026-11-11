// @xpute/runtime/io/batch.spec.ts
//
// GENERATED from spec/io/batch.json
//
// A device's runtime hands its UI what the core sent as one batch (xpute-runtime
// native/serve.rs `take`, @xpute/runtime start.native.ts). Each frame in it is HEADER_WORDS
// little-endian words, TAG to NAMED their indices, then its packet and then the memory it
// names, each padded to ALIGN bytes. Two commands no core sends are the runtime's own: STOPPED
// and COMMIT.

import type { u32 } from "@xpute/kit/abi/word.ts";

/** The frame's tag. */
export const TAG: u32 = 0 as number;

/** The frame's command, without its flags. */
export const CMD: u32 = 1 as number;

/** The frame's flags. */
export const FLAGS: u32 = 2 as number;

/** The frame's result, a value or a negative errno. */
export const RESULT: u32 = 3 as number;

/** The length in bytes of the frame's packet. */
export const PKT: u32 = 4 as number;

/** The length in bytes of the memory the frame names. */
export const NAMED: u32 = 5 as number;

/** The words before a frame's packet. */
export const HEADER_WORDS: u32 = 6 as number;

/** What a frame's packet and the memory it names are each padded to, in bytes. */
export const ALIGN: u32 = 8 as number;

/** The command of the frame that says the core has stopped; it is the last the UI hears. */
export const STOPPED: u32 = 4294967295 as number;

/** The command of the frame that asks the UI to commit a written file, tagged with its
 * descriptor: its packet is the file's path, and the memory it names is the whole file. The
 * UI answers with the close's completion, FS_CLOSE flagged RES. */
export const COMMIT: u32 = 4294967294 as number;
