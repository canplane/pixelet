// @xpute/runtime/ipc/head/word.spec.ts
//
// GENERATED from spec/ipc/head.json

/** Where MAGIC is. */
export const MAGIC = 0 as number;

/** Frames the submission ring holds. */
export const SQ_CAP = 1 as number;

/** Frames the completion ring holds. */
export const CQ_CAP = 2 as number;

/** A ring slot's payload, the same for both rings. */
export const SLOT_BYTES = 3 as number;

/** Where the submission ring's words begin. */
export const SQ_AT = 4 as number;

/** Where the completion ring's words begin. */
export const CQ_AT = 5 as number;

/** Where the GPU stream is, which the runtime runs after each turn; 0 where the machine asks for no GPU. */
export const GPU_AT = 6 as number;
