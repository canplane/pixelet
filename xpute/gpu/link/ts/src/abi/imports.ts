// @xpute/gpu-link/abi/imports.ts
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

import type { Handle } from "@xpute/link/link.ts";

/** The interfaces the world imports that the host serves under a named
 * handle, each `[name, handle]`, which are mounted before the guest boots. */
export const IMPORTS: readonly (readonly [string, Handle])[] = [
  ["xpute:gpu/gpu", 0x79d455],
  ["xpute:gpu/vsync", 0x42eda6],
];
