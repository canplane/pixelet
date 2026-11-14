// @xpute/link/abi/clocks/imports.ts
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

import type { Handle } from "../../link.ts";

/** The interfaces the world imports that the host serves under a named
 * handle, each `[name, handle]`, which are mounted before the guest boots. */
export const IMPORTS: readonly (readonly [string, Handle])[] = [
  ["xpute:clocks/monotonic-clock", 0x73f703],
  ["xpute:clocks/system-clock", 0x7dfa61],
];
