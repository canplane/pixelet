// @xpute/gpu-link/handle.ts

/**
 * A handle is a slot and that slot's generation, so a handle kept past its
 * release names nothing that takes the slot later; the host reads such a
 * handle's slot alone (xpute-gpu-link handle.rs).
 */

import type { u32 } from "@xpute/kit/abi/word.ts";
import { SLOT_BITS } from "./api.spec.ts";

/** The slot of a handle: its low `SLOT_BITS`. */
export const handle_slot = (handle: u32): u32 =>
  handle & ((1 << SLOT_BITS) - 1);
