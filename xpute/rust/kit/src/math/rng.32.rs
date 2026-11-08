// xpute-kit/math/rng.32.rs

//! The 32-bit family, the counterpart of `rng.64.rs` in `u32` arithmetic
//! alone. WebGPU has no 64-bit integer, so a shader can compute these same
//! numbers. The family holds Murmur3's finalizer, a seed derived through
//! it, and a stream that applies it to a Weyl sequence.

/// The golden ratio as a 32-bit fraction, the counterpart of the 64-bit
/// family's salt.
pub const SEED_SALT_32: u32 = 0x9e37_79b9;
/// The two multipliers of Murmur3's finalizer, which `mix32` uses.
pub const MIX32_FIRST: u32 = 0x85eb_ca6b;
pub const MIX32_LAST: u32 = 0xc2b2_ae35;

/// Murmur3's 32-bit finalizer, applied after the salt is added; the
/// counterpart of `mix64`.
pub fn mix32(x: u32) -> u32 {
    let x = x.wrapping_add(SEED_SALT_32);
    let mut z = x;
    z = (z ^ (z >> 16)).wrapping_mul(MIX32_FIRST);
    z = (z ^ (z >> 13)).wrapping_mul(MIX32_LAST);
    z ^ (z >> 16)
}

/// Derives a seed from `base` and `tag` through `mix32`, as
/// `derive_seed_u64` does through `mix64`.
pub fn derive_seed_u32(base: u32, tag: u32) -> u32 {
    mix32(base ^ tag)
}

/// The counterpart of `SplitMix64`. Each draw is `mix32` of a Weyl sequence
/// that steps by the salt, so the stream is the finalizer applied to a
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

#[cfg(test)]
#[path = "rng.32.test.rs"]
mod test;
