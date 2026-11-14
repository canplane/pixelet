// @xpute/gpu-link/abi/later.ts
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

import type { Handle, Method } from "@xpute/link/link.ts";
import * as gpu from "./gpu.ts";
import * as vsync from "./vsync.ts";

/** The methods of the named interfaces the host serves that answer
 * later, each `[handle, method]`, as WIT's `async func` marks them. */
export const LATER: readonly (readonly [Handle, Method])[] = [
  [gpu.HANDLE, gpu.RUN],
  [vsync.HANDLE, vsync.NEXT],
];
