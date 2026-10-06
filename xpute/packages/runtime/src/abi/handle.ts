// @xpute/runtime/abi/handle.ts

/** A guest handle is `slot | generation << 16`, as `xpute_runtime::abi::handle` hands it out. */

import type { u32 } from "@xpute/core/abi/word.ts";

const SLOT_MASK = 0xffff;

export function handle_slot(handle: u32): u32 {
  return handle & SLOT_MASK;
}
