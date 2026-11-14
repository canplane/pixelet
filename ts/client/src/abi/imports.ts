// @pxl/client/abi/imports.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import type { Handle } from "@xpute/link/link.ts";
import * as file from "@xpute/link/abi/fs/file.ts";
import * as gpu from "@xpute/gpu-link/abi/gpu.ts";
import * as monotonic_clock from "@xpute/link/abi/clocks/monotonic_clock.ts";
import * as object from "./object.ts";
import * as overlay from "./overlay.ts";
import * as stop from "@xpute/link/abi/stop/stop.ts";
import * as system_clock from "@xpute/link/abi/clocks/system_clock.ts";
import * as text from "./text.ts";
import * as vsync from "@xpute/gpu-link/abi/vsync.ts";
import * as watch from "./watch.ts";

/** The interfaces the world imports, each `[name, handle]`, which are
 * mounted before the guest boots: one with functions of its own under its
 * handle, one of resources alone by an opener at a prefix. */
export const IMPORTS: readonly (readonly [string, Handle])[] = [
  ["xpute:stop/stop", stop.HANDLE],
  ["xpute:clocks/monotonic-clock", monotonic_clock.HANDLE],
  ["xpute:clocks/system-clock", system_clock.HANDLE],
  ["xpute:fs/file", file.HANDLE],
  ["xpute:gpu/gpu", gpu.HANDLE],
  ["xpute:gpu/vsync", vsync.HANDLE],
  ["pxl:core/object", object.HANDLE],
  ["pxl:core/overlay", overlay.HANDLE],
  ["pxl:core/watch", watch.HANDLE],
  ["pxl:core/text", text.HANDLE],
];
