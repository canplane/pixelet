// @xpute/gpu-link/abi/imports.ts
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

import type { Handle } from "@xpute/link/link.ts";
import * as gpu from "./gpu.ts";
import * as vsync from "./vsync.ts";

/** The interfaces the world imports, each `[name, handle]`, which are
 * mounted before the guest boots: one with functions of its own under its
 * handle, one of resources alone by an opener at a prefix. */
export const IMPORTS: readonly (readonly [string, Handle])[] = [
  ["xpute:gpu/gpu", gpu.HANDLE],
  ["xpute:gpu/vsync", vsync.HANDLE],
];
