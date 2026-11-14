// @xpute/link/abi/clocks/imports.ts
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

import type { Handle } from "../../link.ts";
import * as monotonic_clock from "./monotonic_clock.ts";
import * as system_clock from "./system_clock.ts";

/** The interfaces the world imports, each `[name, handle]`, which are
 * mounted before the guest boots: one with functions of its own under its
 * handle, one of resources alone by an opener at a prefix. */
export const IMPORTS: readonly (readonly [string, Handle])[] = [
  ["xpute:clocks/monotonic-clock", monotonic_clock.HANDLE],
  ["xpute:clocks/system-clock", system_clock.HANDLE],
];
