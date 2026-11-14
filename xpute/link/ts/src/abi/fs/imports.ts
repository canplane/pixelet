// @xpute/link/abi/fs/imports.ts
//
// GENERATED from wit/fs (xpute-link-gen wit.rs)

import type { Handle } from "../../link.ts";

/** The interfaces the world imports that the host serves under a named
 * handle, each `[name, handle]`, which are mounted before the guest boots. */
export const IMPORTS: readonly (readonly [string, Handle])[] = [];
