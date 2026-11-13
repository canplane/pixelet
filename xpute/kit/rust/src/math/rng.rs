// xpute-kit/math/rng.rs

//! These seeds and streams are reproducible, and not for anything that must
//! resist prediction. There are two families: a 64-bit one built on
//! splitmix64, and a 32-bit one built on Murmur3's finalizer, which a GPU
//! shader can compute as well.

/// Maps a `u32` onto [0, 1), uniformly.
pub fn u32_to_unit(x: u32) -> f64 {
    x as f64 / 4_294_967_296.0
}

// ---- the 32-bit family ----

// The 32-bit family is the counterpart of the 64-bit family in `u32` arithmetic
// alone: WebGPU has no 64-bit integer, so a shader can compute these same
// numbers. The family holds Murmur3's finalizer, a seed derived through it,
// and a stream that applies it to a Weyl sequence.

/// The golden ratio as a 32-bit fraction is the counterpart of the 64-bit
/// family's salt.
pub const SEED_SALT_32: u32 = 0x9e37_79b9;
/// These are the two multipliers of Murmur3's finalizer, which `mix32` uses.
pub const MIX32_FIRST: u32 = 0x85eb_ca6b;
pub const MIX32_LAST: u32 = 0xc2b2_ae35;

/// Murmur3's 32-bit finalizer, applied after the salt is added, is the
/// counterpart of `mix64`.
pub fn mix32(x: u32) -> u32 {
    let x = x.wrapping_add(SEED_SALT_32);
    let mut z = x;
    z = (z ^ (z >> 16)).wrapping_mul(MIX32_FIRST);
    z = (z ^ (z >> 13)).wrapping_mul(MIX32_LAST);
    z ^ (z >> 16)
}

/// Derives a seed from `base` and `tag` through `mix32`, as `seed_mix64`
/// does through `mix64`.
pub fn seed_mix32(base: u32, tag: u32) -> u32 {
    mix32(base ^ tag)
}

/// This is the counterpart of `SplitMix64`. Each draw is `mix32` of a Weyl
/// sequence that steps by the salt, so the stream is the finalizer applied to a
/// counter, as splitmix64's is.
#[derive(Clone, Copy, Debug)]
pub struct SplitMix32 {
    x: u32,
}

impl SplitMix32 {
    pub const fn new(seed: u32) -> SplitMix32 {
        SplitMix32 { x: seed }
    }

    pub fn next_u32(&mut self) -> u32 {
        let z = mix32(self.x);
        self.x = self.x.wrapping_add(SEED_SALT_32);
        z
    }
}

// ---- the 64-bit family ----

// The 64-bit family holds splitmix64's finalizer, a seed derived through it,
// and splitmix64's stream, drawn as `u32` from the high 32 bits of each word.

const SEED_SALT_64: u64 = 0x9e3779b97f4a7c15;

/// splitmix64's finalizer applied to `x` plus the golden-ratio salt, which
/// is one splitmix64 step from the state `x`.
pub fn mix64(x: u64) -> u64 {
    let x = x.wrapping_add(SEED_SALT_64);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

pub fn seed_mix64(base: u64, tag: u64) -> u64 {
    mix64(base ^ tag)
}

/// This is splitmix64's stream, drawn as `u32`: each draw is the high 32 bits
/// of the next 64-bit output.
#[derive(Clone, Copy, Debug)]
pub struct SplitMix64 {
    x: u64,
}

impl SplitMix64 {
    pub const fn new(seed: u64) -> SplitMix64 {
        SplitMix64 { x: seed }
    }

    pub fn next_u32(&mut self) -> u32 {
        self.x = self.x.wrapping_add(0x9e3779b97f4a7c15);

        let mut z = self.x;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^= z >> 31;

        (z >> 32) as u32
    }
}

#[cfg(test)]
#[path = "rng.test.rs"]
mod test;
