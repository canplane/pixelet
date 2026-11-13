// @xpute/kit/math/rng.ts

/**
 * These seeds and streams are reproducible, and not for anything that must
 * resist prediction. There are two families: a 64-bit one built on
 * splitmix64, and a 32-bit one built on Murmur3's finalizer, which a GPU
 * shader can compute as well.
 */

import { U32, U64, type f64, type u32, type u64 } from "../abi/word.ts";

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

// ---- the 32-bit family ----

// The 32-bit family is the counterpart of the 64-bit one in `u32`
// arithmetic alone: WebGPU has no 64-bit integer, so a shader can compute
// these same numbers. The family holds Murmur3's finalizer, a seed derived
// through it, and a stream that applies it to a Weyl sequence.

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
 * Derives a seed from `base` and `tag` through `mix32`, as `seed_mix64`
 * does through `mix64`.
 */
export function seed_mix32(base: u32, tag: u32): u32 {
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

// ---- the 64-bit family ----

// The 64-bit family holds splitmix64's finalizer, a seed derived through it,
// and splitmix64's stream, drawn as `u32` from the high 32 bits of each word.

/**
 * [Seed Generator]
 *
 * Generates a 64-bit seed for randomized iteration / shuffling / sampling.
 *
 * SECURITY NOTICE:
 * - This is NOT a cryptographic RNG API.
 * - The returned seed is intended for load distribution, randomized routing order,
 *   probabilistic scheduling, etc.
 * - `crypto.getRandomValues` is its entropy source.
 *
 * DO NOT use this seed for:
 * - key generation
 * - nonce generation
 * - session tokens
 * - signatures / encryption
 * - authentication / authorization
 *
 * For cryptographic purposes, always use a dedicated CSPRNG.
 */
export function seed64(): u64 {
  const u = new Uint32Array(2);
  crypto.getRandomValues(u);
  return (BigInt(u[0]) << 32n) | BigInt(u[1]);
}

const SEED_SALT_64 = 0x9e3779b97f4a7c15n;

/**
 * 64-bit mixing function (splitmix64-style).
 *
 * Good avalanche properties for deriving child seeds from a parent seed.
 * NOT cryptographically secure.
 *
 * Use cases:
 * - Derive separate random streams from a single master seed
 * - Create deterministic per-component seeds
 * - Hash-based seed mixing
 *
 * @param x - Input seed
 * @returns Mixed 64-bit value with good statistical properties
 *
 * @example
 * const master = seed64();
 * const seed_a = mix64(master ^ 1n);
 * const seed_b = mix64(master ^ 2n);
 * // seed_a and seed_b are uncorrelated in practice
 */
export function mix64(x: u64): u64 {
  x = U64(x + SEED_SALT_64);
  let z: u64 = x;
  z = U64((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n);
  z = U64((z ^ (z >> 27n)) * 0x94d049bb133111ebn);
  return U64(z ^ (z >> 31n));
}

/**
 * Derive a child seed from a parent seed and a tag.
 *
 * Creates deterministic child seeds that are uncorrelated in practice with each
 * other. Useful for creating separate random streams for different components
 * while maintaining reproducibility from a single master seed.
 *
 * @param base - Parent seed
 * @param tag - Tag to differentiate child seeds (e.g., component ID, role index)
 * @returns Derived seed
 *
 * @example
 * // Derive separate seeds for different roles
 * const master = 12345n;
 * const seed_server = seed_mix64(master, 0n);   // for SERVER role
 * const seed_storage = seed_mix64(master, 1n);  // for STORAGE role
 * // seed_server and seed_storage are uncorrelated in practice
 *
 * @example
 * // Chain derivations for hierarchical streams
 * const master = seed64();
 * const node_seed = seed_mix64(master, 1n);
 * const treap_seed = seed_mix64(node_seed, 0n);
 * const sampling_seed = seed_mix64(node_seed, 1n);
 */
export function seed_mix64(base: u64, tag: u64): u64 {
  return mix64(U64(base ^ U64(tag)));
}

/**
 * Create a deterministic RNG from a seed.
 *
 * Implements splitmix64 algorithm, returning the high 32 bits of each iteration.
 * Provides good statistical properties for non-cryptographic use cases.
 *
 * Use cases:
 * - Treap priorities (deterministic tree shapes)
 * - Randomized load balancing (reproducible decisions)
 * - Shuffling / sampling (deterministic ordering)
 * - Testing (reproducible random behavior)
 *
 * @param seed - Initial seed (default: seed64() for non-deterministic)
 * @returns RNG function that produces u32 values
 *
 * @example
 * // Deterministic RNG
 * const rng = make_rng(12345n);
 * console.log(rng());  // always same sequence
 *
 * @example
 * // Non-deterministic RNG (default)
 * const rng = make_rng();  // uses seed64()
 * console.log(rng());  // different each run
 *
 * @example
 * // Derive separate RNG streams
 * const master = seed64();
 * const rng_a = make_rng(seed_mix64(master, 0n));
 * const rng_b = make_rng(seed_mix64(master, 1n));
 * // rng_a and rng_b are separate windows of one cycle, uncorrelated in practice, and reproducible
 */
export function make_rng(seed: u64 = seed64()): RNG {
  let x: u64 = U64(seed);

  return () => {
    // advance internal state (mod 2^64)
    x = U64(x + 0x9e3779b97f4a7c15n);

    // scramble bits (avalanche)
    let z: u64 = x;
    z = U64((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n);
    z = U64((z ^ (z >> 27n)) * 0x94d049bb133111ebn);
    z ^= z >> 31n;

    // The shift happens while `z` is a bigint, so the 32 bits that remain fit
    // a `Number` exactly.
    return Number(z >> 32n);
  };
}
