// @xpute/kit/math/rng.ts

import { type f64, U32, type u32, U64, type u64 } from "../abi/word.ts";

/** Web Crypto entropy, for shuffling and sampling; not for anything that must resist prediction. */
export function seed64(): u64 {
  const u = new Uint32Array(2);
  crypto.getRandomValues(u);
  return (BigInt(u[0]) << 32n) | BigInt(u[1]);
}

const SEED_SALT = 0x9e3779b97f4a7c15n;

/** splitmix64's finalizer, for deriving independent child seeds; not cryptographic. */
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

export type RNG = () => u32;

/** splitmix64; not cryptographic. */
export function make_rng(seed: u64 = seed64()): RNG {
  let x: u64 = U64(seed);

  return () => {
    x = U64(x + 0x9e3779b97f4a7c15n);

    let z: u64 = x;
    z = U64((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n);
    z = U64((z ^ (z >> 27n)) * 0x94d049bb133111ebn);
    z ^= z >> 31n;

    // Not the low 32 bits: `Number` rounds past 2^53 first. The Rust and C++
    // draw it the same way, and golden/math/rng.tsv holds this sequence.
    return U32(Number(z));
  };
}

export function u32_to_unit(x: u32): f64 {
  return x / 0x100000000;
}
