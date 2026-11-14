// @pxl/client/abi/imports.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import type { Handle } from "@xpute/link/link.ts";

/** The interfaces the world imports that the host serves under a named
 * handle, each `[name, handle]`, which are mounted before the guest boots. */
export const IMPORTS: readonly (readonly [string, Handle])[] = [
  ["xpute:stop/stop", 0x4eccaa],
  ["xpute:clocks/monotonic-clock", 0x73f703],
  ["xpute:clocks/system-clock", 0x7dfa61],
  ["xpute:gpu/gpu", 0x79d455],
  ["xpute:gpu/vsync", 0x42eda6],
  ["pxl:core/object", 0x4edab0],
  ["pxl:core/overlay", 0x7b1393],
  ["pxl:core/watch", 0x71919e],
  ["pxl:core/text", 0x61d944],
];
