// @xpute/host/abi/fs/imports.ts
//
// GENERATED from wit/fs (xpute-link-gen wit.rs)

import type { Handle } from "@xpute/link/link.ts";

/** The interfaces the world imports, each `[name, handle]`, which are
 * mounted before the guest boots: one with functions of its own under its
 * handle, one of resources alone by an opener at a prefix. */
export const IMPORTS: readonly (readonly [string, Handle])[] = [
  ["xpute:fs/file", 0x5b6743],
];
