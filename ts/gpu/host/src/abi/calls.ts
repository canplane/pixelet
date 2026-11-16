// @pxl/gpu/host/src/abi/calls.ts
//
// GENERATED from spec/gpu/wit (xpute-link-gen wit.rs)

import type { Handle, Method } from "@xpute/link/link.ts";

/** The methods of the interfaces the world imports that answer at once,
 * each `[handle, method]`, a function without WIT's `async`, which a
 * runtime refuses to carry to a UI on another thread. */
export const CALLS: readonly (readonly [Handle, Method])[] = [
  [0x54bd98, 1],
  [0x73f703, 0],
];
