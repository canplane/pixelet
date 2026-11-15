// @pxl/client/abi/imports.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import type { Handle } from "@xpute/link/link.ts";

/** The interfaces the world imports, each `[name, handle]`, which are
 * mounted before the guest boots: one with functions of its own under its
 * handle, one of resources alone by an opener at a prefix. */
export const IMPORTS: readonly (readonly [string, Handle])[] = [
  ["xpute:stop/stop", 0x4eccaa],
  ["xpute:clocks/monotonic-clock", 0x73f703],
  ["xpute:clocks/system-clock", 0x7dfa61],
  ["xpute:fs/file", 0x5b6743],
  ["pxl:gpu/gpu", 0x54bd98],
  ["pxl:gpu/vsync", 0x51cb2a],
  ["pxl:core/object", 0x4edab0],
  ["pxl:core/overlay", 0x7b1393],
  ["pxl:core/text", 0x61d944],
];
