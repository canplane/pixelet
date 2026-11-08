// @xpute/runtime/abi/handle.ts

/** A core handle is `slot | generation << SLOT_BITS`, as `xpute_core::abi::handle` hands it out. */

import type { u32 } from "@xpute/kit/abi/word.ts";

/** A copy of the core's `SLOT_BITS` (xpute-core abi/handle.rs). The conformance
 * traces' `slot` relations catch a value here that is wider than the core's,
 * but never a narrower one. */
export const SLOT_BITS = 16;
/** The width of the core's generation, a `u16` (xpute-core abi/handle.rs
 * `Slot`), copied likewise. The two widths together fill the handle's word,
 * as handle.test.ts checks. */
export const GENERATION_BITS = 16;
const SLOT_MASK = (1 << SLOT_BITS) - 1;

export function handle_slot(handle: u32): u32 {
  return handle & SLOT_MASK;
}
