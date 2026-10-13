// xpute-kit/math/rng.rs

//! Reproducible seeds and streams; not for anything that must resist prediction.
//! Two families: 64-bit (splitmix64) and 32-bit (Murmur3's finalizer), the
//! latter gpu-portable, since WebGPU has no 64-bit integer.

const SEED_SALT: u64 = 0x9e3779b97f4a7c15;

/// The golden ratio's 32-bit word, `SEED_SALT`'s counterpart.
const SEED_SALT_32: u32 = 0x9e37_79b9;

/// splitmix64's finalizer.
pub fn mix64(x: u64) -> u64 {
    let x = x.wrapping_add(SEED_SALT);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

pub fn derive_seed_u64(base: u64, tag: u64) -> u64 {
    mix64(base ^ tag)
}

/// gpu-portable: Murmur3's 32-bit finalizer, `mix64`'s counterpart.
pub fn mix32(x: u32) -> u32 {
    let x = x.wrapping_add(SEED_SALT_32);
    let mut z = x;
    z = (z ^ (z >> 16)).wrapping_mul(0x85eb_ca6b);
    z = (z ^ (z >> 13)).wrapping_mul(0xc2b2_ae35);
    z ^ (z >> 16)
}

/// gpu-portable: `derive_seed_u64`'s counterpart over `mix32`.
pub fn derive_seed_u32(base: u32, tag: u32) -> u32 {
    mix32(base ^ tag)
}

/// splitmix64, drawn as u32.
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

        // Not `z as u32`: rounding through an f64 is the sequence
        // `spec/golden/math/rng.tsv` holds for every language.
        ((z as f64) % 4_294_967_296.0) as u32
    }
}

/// Uniform in [0, 1).
pub fn u32_to_unit(x: u32) -> f64 {
    x as f64 / 4_294_967_296.0
}

#[cfg(test)]
#[path = "rng.test.rs"]
mod test;
