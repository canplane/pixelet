// @xpute/kit/math/rng.64.ts

/**
 * The 64-bit family holds splitmix64's finalizer, a seed derived through it,
 * and splitmix64's stream, drawn as `u32` from the high 32 bits of each word.
 */

import { U64, type u64 } from "../abi/word.ts";
import type { RNG } from "./rng.ts";

/**
 * This is a seed from Web Crypto's entropy, for shuffling and sampling. The
 * streams it seeds must still not be used where prediction must be resisted.
 */
export function seed64(): u64 {
  const u = new Uint32Array(2);
  crypto.getRandomValues(u);
  return (BigInt(u[0]) << 32n) | BigInt(u[1]);
}

const SEED_SALT = 0x9e3779b97f4a7c15n;

/**
 * splitmix64's finalizer applied to `x` plus the golden-ratio salt, which is
 * one splitmix64 step from the state `x`.
 */
export function mix64(x: u64): u64 {
  x = U64(x + SEED_SALT);
  let z: u64 = x;
  z = U64((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n);
  z = U64((z ^ (z >> 27n)) * 0x94d049bb133111ebn);
  return U64(z ^ (z >> 31n));
}

export function derive_seed_u64(base: u64, tag: u64): u64 {
  return mix64(U64(base ^ U64(tag)));
}

/**
 * This is splitmix64's stream, drawn as `u32`: each draw is the high 32 bits of
 * the next 64-bit output.
 */
export function make_rng(seed: u64 = seed64()): RNG {
  let x: u64 = U64(seed);

  return () => {
    x = U64(x + 0x9e3779b97f4a7c15n);

    let z: u64 = x;
    z = U64((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n);
    z = U64((z ^ (z >> 27n)) * 0x94d049bb133111ebn);
    z ^= z >> 31n;

    // The shift happens while `z` is a bigint, so the 32 bits that remain fit
    // a `Number` exactly.
    return Number(z >> 32n);
  };
}
