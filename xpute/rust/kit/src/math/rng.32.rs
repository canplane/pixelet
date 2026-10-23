// xpute-kit/math/rng.32.rs

//! gpu-portable: the 32-bit family, `rng.64.rs`'s counterpart in u32 alone,
//! since WebGPU has no 64-bit integer: Murmur3's finalizer, a seed derived
//! through it, and a stream that draws it over a Weyl sequence.

/// The golden ratio's 32-bit word, the 64-bit family's salt's counterpart.
const SEED_SALT_32: u32 = 0x9e37_79b9;

/// Murmur3's 32-bit finalizer, `mix64`'s counterpart.
pub fn mix32(x: u32) -> u32 {
    let x = x.wrapping_add(SEED_SALT_32);
    let mut z = x;
    z = (z ^ (z >> 16)).wrapping_mul(0x85eb_ca6b);
    z = (z ^ (z >> 13)).wrapping_mul(0xc2b2_ae35);
    z ^ (z >> 16)
}

/// `derive_seed_u64`'s counterpart over `mix32`.
pub fn derive_seed_u32(base: u32, tag: u32) -> u32 {
    mix32(base ^ tag)
}

/// `SplitMix64`'s counterpart: each draw `mix32` of a Weyl sequence that
/// steps by the salt, so the stream is the finalizer over a counter, as
/// splitmix64's is.
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
