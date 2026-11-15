// @pxl/client/abi/calls.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import type { Handle, Method } from "@xpute/link/link.ts";

/** The methods of the interfaces the world imports that answer at once,
 * each `[handle, method]`, a function without WIT's `async`, which a
 * runtime refuses to carry to a UI on another thread. */
export const CALLS: readonly (readonly [Handle, Method])[] = [
  [0x4eccaa, 0],
  [0x73f703, 0],
  [0x7dfa61, 0],
  [0x54bd98, 1],
  [0x61d944, 0],
  [0x61d944, 1],
];
