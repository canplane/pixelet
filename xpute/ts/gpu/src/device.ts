// @xpute/gpu/device.ts

/**
 * These are the word offsets of the device's block (spec DEVICE), as
 * rust/gpu/src/device.rs has them. The runtime writes the block where the
 * core asks for it, and the core reads it. The limits come first, one word for
 * each name in LIMIT_NAME.
 */

import { FEATURE_NAME, LIMIT_NAME } from "./enum.spec.ts";

/** The features take one bit for each name in FEATURE_NAME. */
export const FEATS_AT = LIMIT_NAME.length;
/** The timestamp period is in nanoseconds per tick, as f32 bits. */
export const PERIOD_AT = FEATS_AT + Math.ceil(FEATURE_NAME.length / 32);
/** The surface format the runtime prefers is an index into TEXTURE_FORMAT. */
export const FORMAT_AT = PERIOD_AT + 1;
/** This counts the uncaptured errors the device raised, and the runtime keeps
 * it up to date. */
export const ERRORS_AT = FORMAT_AT + 1;
/** This counts the core's frames the GPU has not yet finished, and the runtime
 * keeps it up to date. */
export const UNFINISHED_AT = ERRORS_AT + 1;
/** This is the device's STATUS, the block's last word. The runtime stores it
 * with release after the rest of the block, so a core that loads READY with
 * acquire sees the whole block. */
export const STATUS_AT = UNFINISHED_AT + 1;
export const WORDS = STATUS_AT + 1;
