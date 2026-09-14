// xpute-core/kit/rng.rs

use core::cell::Cell;

/// [Seed Generator]
///
/// Generates a 64-bit seed for randomized iteration / shuffling / sampling.
///
/// SECURITY NOTICE:
/// - This is NOT a cryptographic RNG API.
/// - The returned seed is intended for load distribution, randomized routing order,
///   probabilistic scheduling, etc.
/// - The TypeScript takes its entropy from `crypto.getRandomValues` when the
///   host has it, and from time and `Math.random` otherwise. The module has
///   no entropy of its own: the host installs a source (`set_entropy`), and
///   until it does the seed is a mixed counter — deterministic, which is
///   NOT secure against prediction or replay.
///
/// DO NOT use this seed for:
/// - key generation
/// - nonce generation
/// - session tokens
/// - signatures / encryption
/// - authentication / authorization
///
/// For cryptographic purposes, always use a dedicated CSPRNG.
pub fn seed64() -> u64 {
    if let Some(f) = ENTROPY.with(|e| e.get()) {
        return f();
    }
    let n = COUNTER.with(|c| {
        let n = c.get();
        c.set(n + 1);
        n
    });
    mix64(n) ^ 0x9e3779b97f4a7c15
}

thread_local! {
    static ENTROPY: Cell<Option<fn() -> u64>> = const { Cell::new(None) };
    static COUNTER: Cell<u64> = const { Cell::new(0) };
}

/// Installs the host's entropy source, what `crypto.getRandomValues` is there.
pub fn set_entropy(f: fn() -> u64) {
    ENTROPY.with(|e| e.set(Some(f)));
}

// ============ Seed Derivation ============

const SEED_SALT: u64 = 0x9e3779b97f4a7c15;

/// 64-bit mixing function (splitmix64-style).
///
/// Good avalanche properties for deriving child seeds from a parent seed.
/// NOT cryptographically secure.
///
/// Use cases:
/// - Derive independent random streams from a single master seed
/// - Create deterministic per-component seeds
/// - Hash-based seed mixing
pub fn mix64(x: u64) -> u64 {
    let x = x.wrapping_add(SEED_SALT);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

/// Derive a child seed from a parent seed and a tag.
///
/// Creates deterministic child seeds that are statistically independent from each
/// other. Useful for creating separate random streams for different components
/// while maintaining reproducibility from a single master seed.
pub fn derive_seed_u64(base: u64, tag: u64) -> u64 {
    mix64(base ^ tag)
}

// ============ Deterministic RNG ============

/// Deterministic pseudo-random number generator.
///
/// Returns u32 values [0, 2^32) with good statistical properties.
/// NOT cryptographically secure.
pub type RNG = Box<dyn FnMut() -> u32>;

/// Create a deterministic RNG from a seed.
///
/// Implements splitmix64 algorithm, returning the low 32 bits of each iteration.
/// Provides good statistical properties for non-cryptographic use cases.
///
/// Use cases:
/// - Treap priorities (deterministic tree shapes)
/// - Randomized load balancing (reproducible decisions)
/// - Shuffling / sampling (deterministic ordering)
/// - Testing (reproducible random behavior)
///
/// `seed` defaults to `seed64()` for a non-deterministic stream.
pub fn make_rng(seed: Option<u64>) -> RNG {
    let mut x: u64 = seed.unwrap_or_else(seed64);

    Box::new(move || {
        // advance internal state (mod 2^64)
        x = x.wrapping_add(0x9e3779b97f4a7c15);

        // scramble bits (avalanche)
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^= z >> 31;

        // return low 32-bit — of the word as a JS number: `U32(Number(z))`
        // rounds the 64-bit word to the nearest double first, then takes it
        // modulo 2^32, which is not the word's low bits once it is past 2^53.
        ((z as f64) % 4_294_967_296.0) as u32
    })
}

/// Convert u32 random value to uniform float in [0, 1).
pub fn u32_to_unit(x: u32) -> f64 {
    x as f64 / 4_294_967_296.0
}
