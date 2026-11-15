// @xpute/host/abi/clocks/imports.ts
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

import type { Handle } from "@xpute/link/link.ts";

/** The interfaces the world imports, each `[name, handle]`, which are
 * mounted before the guest boots: one with functions of its own under its
 * handle, one of resources alone by an opener at a prefix. */
export const IMPORTS: readonly (readonly [string, Handle])[] = [
  ["xpute:clocks/monotonic-clock", 0x73f703],
  ["xpute:clocks/system-clock", 0x7dfa61],
];
