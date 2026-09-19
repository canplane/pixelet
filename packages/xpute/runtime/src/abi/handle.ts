// @xpute/runtime/abi/handle.ts

/**
 * The host's side of a handle: the guest's table (`xpute_runtime::abi::handle`)
 * hands out `slot | generation << 16`, and the host reads the slot off one to
 * index what it keeps under it. The 16 is the table's, written here once more
 * rather than generated: it is half of a u32 and the generation lane is `u16`
 * to match, so it is not a number anyone turns.
 */

import type { u32 } from "@xpute/core/abi/word.ts";

const SLOT_MASK = 0xffff;

/** The slot a handle names. */
export function handle_slot(handle: u32): u32 {
  return handle & SLOT_MASK;
}
