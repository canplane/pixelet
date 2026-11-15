// @pxl/client/abi/later.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import type { Handle, Method } from "@xpute/link/link.ts";
import * as object from "./object.ts";

/** The methods of the package's own named interfaces its world imports
 * that answer later, each `[handle, method]`, as WIT's `async func`
 * marks them. */
export const LATER: readonly (readonly [Handle, Method])[] = [
  [object.HANDLE, object.APPLY],
];
