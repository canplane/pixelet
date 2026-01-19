// @xpute/spec/lib/kit/rng.ts

import { type f64, U32, type u32, U64, type u64 } from "./word.ts";

/**
 * [Seed Generator]
 *
 * Generates a 64-bit seed for randomized iteration / shuffling / sampling.
 *
 * SECURITY NOTICE:
 * - This is NOT a cryptographic RNG API.
 * - The returned seed is intended for load distribution, randomized routing order,
 *   probabilistic scheduling, etc.
 * - If the environment provides `crypto.getRandomValues`, it is used as the primary
 *   entropy source (strong randomness).
 * - Otherwise, it falls back to weaker entropy sources such as time and Math.random,
 *   which are NOT secure against prediction or replay.
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
  // 1) crypto.getRandomValues (Browser + Node >= 15)
  const g = globalThis;
  const c = g.crypto;

  if (c?.getRandomValues) {
    const u = new Uint32Array(2);
    c.getRandomValues(u);
    const hi = u[0];
    const lo = u[1];
    return (BigInt(hi) << 32n) | BigInt(lo);
  }

  // 2) Node fallback (require 없이도 최대한)
  //    (randomUUID 있으면 거기서 엔트로피 뽑기)
  if (c?.randomUUID) {
    const s = c.randomUUID().replace(/-/g, "");
    return BigInt("0x" + s.slice(0, 16));
  }

  // 3) Ultimate fallback: time + Math.random (weak but universal)
  const t = BigInt(Date.now());
  const r = BigInt(Math.floor(Math.random() * 0x100000000));
  return (t << 32n) ^ (r << 1n) ^ 0x9e3779b97f4a7c15n;
}

// ============ Seed Derivation ============

const SEED_SALT = 0x9e3779b97f4a7c15n;

/**
 * 64-bit mixing function (splitmix64-style).
 *
 * Good avalanche properties for deriving child seeds from a parent seed.
 * NOT cryptographically secure.
 *
 * Use cases:
 * - Derive independent random streams from a single master seed
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
 * // seed_a and seed_b are independent
 */
export function mix64(x: u64): u64 {
  x = U64(x + SEED_SALT);
  let z: u64 = x;
  z = U64((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n);
  z = U64((z ^ (z >> 27n)) * 0x94d049bb133111ebn);
  return U64(z ^ (z >> 31n));
}

/**
 * Derive a child seed from a parent seed and a tag.
 *
 * Creates deterministic child seeds that are statistically independent from each
 * other. Useful for creating separate random streams for different components
 * while maintaining reproducibility from a single master seed.
 *
 * @param base - Parent seed
 * @param tag - Tag to differentiate child seeds (e.g., component ID, role index)
 * @returns Derived seed
 *
 * @example
 * // Derive independent seeds for different roles
 * const master = 12345n;
 * const seed_server = derive_seed_u64(master, 0n);   // for SERVER role
 * const seed_storage = derive_seed_u64(master, 1n);  // for STORAGE role
 * // seed_server and seed_storage are statistically independent
 *
 * @example
 * // Chain derivations for hierarchical streams
 * const master = seed64();
 * const node_seed = derive_seed_u64(master, 1n);
 * const treap_seed = derive_seed_u64(node_seed, 0n);
 * const sampling_seed = derive_seed_u64(node_seed, 1n);
 */
export function derive_seed_u64(base: u64, tag: u64): u64 {
  return mix64(U64(base ^ U64(tag)));
}

// ============ Deterministic RNG ============

/**
 * Deterministic pseudo-random number generator.
 *
 * Returns u32 values [0, 2^32) with good statistical properties.
 * NOT cryptographically secure.
 */
export type RNG = () => u32;

/**
 * Create a deterministic RNG from a seed.
 *
 * Implements splitmix64 algorithm, returning the low 32 bits of each iteration.
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
 * // Derive independent RNG streams
 * const master = seed64();
 * const rng_a = make_rng(derive_seed_u64(master, 0n)!);
 * const rng_b = make_rng(derive_seed_u64(master, 1n)!);
 * // rng_a and rng_b are independent but reproducible
 */
export function make_rng(seed: bigint = seed64()): RNG {
  let x: u64 = U64(seed);

  return () => {
    // advance internal state (mod 2^64)
    x = U64(x + 0x9e3779b97f4a7c15n);

    // scramble bits (avalanche)
    let z: u64 = x;
    z = U64((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n);
    z = U64((z ^ (z >> 27n)) * 0x94d049bb133111ebn);
    z ^= z >> 31n;

    // return low 32-bit
    return U32(Number(z));
  };
}

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
