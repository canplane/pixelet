// @xpute/kit/math/rng.32.ts

/**
 * The 32-bit family is the counterpart of rng.64.ts in `u32` arithmetic alone:
 * WebGPU has no 64-bit integer, so a shader can compute these same numbers.
 * The family holds Murmur3's finalizer, a seed derived through it, and a
 * stream that applies it to a Weyl sequence.
 */

import { U32, type u32 } from "../abi/word.ts";
import type { RNG } from "./rng.ts";

const SEED_SALT_32 = 0x9e3779b9;

/** Murmur3's 32-bit finalizer, applied after the salt is added, is the
 * counterpart of `mix64`. */
export function mix32(x: u32): u32 {
  let z = U32(x + SEED_SALT_32);
  z = U32(Math.imul(z ^ (z >>> 16), 0x85ebca6b));
  z = U32(Math.imul(z ^ (z >>> 13), 0xc2b2ae35));
  return U32(z ^ (z >>> 16));
}

/**
 * Derives a seed from `base` and `tag` through `mix32`, as `derive_seed_u64`
 * does through `mix64`.
 */
export function derive_seed_u32(base: u32, tag: u32): u32 {
  return mix32(U32(base ^ tag));
}

/**
 * This is the counterpart of `make_rng`. Each draw is `mix32` of a Weyl
 * sequence that steps by the salt, so the stream is the finalizer applied to a
 * counter, as splitmix64's is.
 */
export function make_rng32(seed: u32): RNG {
  let x: u32 = U32(seed);
  return () => {
    const z = mix32(x);
    x = U32(x + SEED_SALT_32);
    return z;
  };
}
