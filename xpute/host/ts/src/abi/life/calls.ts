// @xpute/host/abi/life/calls.ts
//
// GENERATED from wit/life (xpute-link-gen wit.rs)

import type { Handle, Method } from "@xpute/link/link.ts";

/** The methods of the interfaces the world imports that answer at once,
 * each `[handle, method]`, a function without WIT's `async`, which a
 * runtime refuses to carry to a UI on another thread. */
export const CALLS: readonly (readonly [Handle, Method])[] = [];
