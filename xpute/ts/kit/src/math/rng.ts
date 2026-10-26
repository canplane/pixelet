// @xpute/kit/math/rng.ts

/**
 * Reproducible seeds and streams; not for anything that must resist
 * prediction. Two families, a file each: 64-bit (splitmix64, rng.64.ts) and
 * 32-bit (Murmur3's finalizer, rng.32.ts), the latter gpu-portable, since
 * WebGPU has no 64-bit integer.
 */

import type { f64, u32 } from "../abi/word.ts";

export * from "./rng.32.ts";
export * from "./rng.64.ts";

/** A stream's draws, one u32 a call. */
export type RNG = () => u32;

/** Uniform in [0, 1). */
export function u32_to_unit(x: u32): f64 {
  return x / 0x100000000;
}
