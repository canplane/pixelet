// @xpute/runtime/ipc/head/word.spec.ts
//
// GENERATED from spec/ipc/head.json

export const MAGIC = 0 as number;

/** How many frames the submission ring holds. */
export const SQ_CAP = 1 as number;

/** How many frames the completion ring holds. */
export const CQ_CAP = 2 as number;

/** The size of one ring slot in bytes, the same for both rings. */
export const SLOT_BYTES = 3 as number;

/** The offset in the memory where the submission ring begins. */
export const SQ_AT = 4 as number;

/** The offset in the memory where the completion ring begins. */
export const CQ_AT = 5 as number;

/** The offset of the stream, which is run after each turn; 0 when the machine
 * asks for none. */
export const STREAM_AT = 6 as number;
