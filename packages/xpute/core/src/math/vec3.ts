// @xpute/core/math/vec3.ts

import type { f64 } from "../abi/word.ts";

/** A point or a direction as three doubles, written into by the matrix
 * functions and read by whatever asked. */
export interface Vec3 {
  x: f64;
  y: f64;
  z: f64;
}
