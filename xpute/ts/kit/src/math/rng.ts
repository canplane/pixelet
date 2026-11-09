// @xpute/kit/math/rng.ts

/**
 * These seeds and streams are reproducible, and not for anything that must
 * resist prediction. There are two families, a file each: a 64-bit one built
 * on splitmix64 (rng.64.ts), and a 32-bit one built on Murmur3's finalizer
 * (rng.32.ts), which a GPU shader can compute as well.
 */

import type { f64, u32 } from "../abi/word.ts";

export * from "./rng.32.ts";
export * from "./rng.64.ts";

// ============ Deterministic RNG ============

/**
 * Deterministic pseudo-random number generator.
 *
 * Returns u32 values [0, 2^32) with good statistical properties.
 * NOT cryptographically secure.
 */
export type RNG = () => u32;

/**
 * Convert u32 random value to uniform float in [0, 1).
 *
 * @param x - u32 random value [0, 2^32)
 * @returns Float in [0, 1)
 *
 * @example
 * const rng = make_rng();
 * if (u32_to_unit(rng()) < 0.1) {
 *   // 10% probability
 * }
 */
export function u32_to_unit(x: u32): f64 {
  return x / 0x100000000;
}
