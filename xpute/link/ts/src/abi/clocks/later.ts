// @xpute/link/abi/clocks/later.ts
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

import type { Handle, Method } from "../../link.ts";
import * as monotonic_clock from "./monotonic_clock.ts";

/** The methods of the package's own named interfaces its world imports
 * that answer later, each `[handle, method]`, as WIT's `async func`
 * marks them. */
export const LATER: readonly (readonly [Handle, Method])[] = [
  [monotonic_clock.HANDLE, monotonic_clock.WAIT_FOR],
];
