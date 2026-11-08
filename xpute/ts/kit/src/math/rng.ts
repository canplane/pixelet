// @xpute/kit/math/rng.ts

/**
 * Reproducible seeds and random streams, not for anything that must resist
 * prediction. There are two families, a file each: a 64-bit one built on
 * splitmix64 (rng.64.ts), and a 32-bit one built on Murmur3's finalizer
 * (rng.32.ts), which a GPU shader can compute as well.
 */

import type { f64, u32 } from "../abi/word.ts";

export * from "./rng.32.ts";
export * from "./rng.64.ts";

/** A random stream: each call draws the next `u32`. */
export type RNG = () => u32;

/** Maps a `u32` onto [0, 1), uniformly. */
export function u32_to_unit(x: u32): f64 {
  return x / 0x100000000;
}
