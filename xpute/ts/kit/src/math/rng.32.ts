// @xpute/kit/math/rng.32.ts

/**
 * The 32-bit family, rng.64.ts's counterpart in u32 alone, gpu-portable since
 * WebGPU has no 64-bit integer: Murmur3's finalizer, a seed derived through
 * it, and a stream that draws it over a Weyl sequence. Not cryptographic.
 */

import { U32, type u32 } from "../abi/word.ts";
import type { RNG } from "./rng.ts";

const SEED_SALT_32 = 0x9e3779b9;

/** Murmur3's 32-bit finalizer, `mix64`'s counterpart. */
export function mix32(x: u32): u32 {
  let z = U32(x + SEED_SALT_32);
  z = U32(Math.imul(z ^ (z >>> 16), 0x85ebca6b));
  z = U32(Math.imul(z ^ (z >>> 13), 0xc2b2ae35));
  return U32(z ^ (z >>> 16));
}

/** `derive_seed_u64`'s counterpart over `mix32`. */
export function derive_seed_u32(base: u32, tag: u32): u32 {
  return mix32(U32(base ^ tag));
}

/** `make_rng`'s counterpart: each draw `mix32` of a Weyl sequence that steps by the salt. */
export function make_rng32(seed: u32): RNG {
  let x: u32 = U32(seed);
  return () => {
    const z = mix32(x);
    x = U32(x + SEED_SALT_32);
    return z;
  };
}
