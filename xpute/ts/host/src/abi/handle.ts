// @xpute/host/abi/handle.ts

/** A guest handle is `slot | generation << SLOT_BITS`, as `xpute_guest::abi::handle` hands it out. */

import type { u32 } from "@xpute/kit/abi/word.ts";

/** As the guest's (xpute-guest abi/handle.rs `SLOT_BITS`), copied: the conformance traces'
 * `slot` relations catch only a value here wider than the guest's, never a narrower one. */
export const SLOT_BITS = 16;
/** The guest's generation, a `u16` (xpute-guest abi/handle.rs `Slot`), copied likewise; the
 * two fill the handle's word (handle.test.ts). */
export const GENERATION_BITS = 16;
const SLOT_MASK = (1 << SLOT_BITS) - 1;

export function handle_slot(handle: u32): u32 {
  return handle & SLOT_MASK;
}
