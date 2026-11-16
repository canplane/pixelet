// @pxl/gpu/host/src/abi/imports.ts
//
// GENERATED from spec/gpu/wit (xpute-link-gen wit.rs)

import type { Handle } from "@xpute/link/link.ts";

/** The interfaces the world imports, each `[name, handle]`, which are
 * mounted before the guest boots: one with functions of its own under its
 * handle, one of resources alone by an opener at a prefix. */
export const IMPORTS: readonly (readonly [string, Handle])[] = [
  ["pxl:gpu/gpu", 0x54bd98],
  ["pxl:gpu/vsync", 0x51cb2a],
];
