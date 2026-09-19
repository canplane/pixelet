// xpute-core/math/rng.rs

//! Seeds and the stream drawn from one: reproducible, and for shuffling,
//! sampling and scatter — nothing that must resist prediction.

const SEED_SALT: u64 = 0x9e3779b97f4a7c15;

/// splitmix64's finalizer: enough avalanche that child seeds drawn from one
/// parent are independent of each other.
pub fn mix64(x: u64) -> u64 {
    let x = x.wrapping_add(SEED_SALT);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

/// A child seed: independent of its siblings, and the same one every run for
/// the same parent and tag.
pub fn derive_seed_u64(base: u64, tag: u64) -> u64 {
    mix64(base ^ tag)
}

/// A splitmix64 stream drawn as u32: eight bytes and `Copy`, so a caller
/// seeds one per cell and keeps it on its stack. The same seed draws the same
/// sequence every run.
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

        // Not `z as u32`: the draw goes through an f64, which rounds the word
        // to the nearest double before the modulo, so past 2^53 it is not the
        // word's low bits. This sequence is what `golden/math/rng.tsv` holds
        // and what the decor scatter is placed from.
        ((z as f64) % 4_294_967_296.0) as u32
    }
}

/// A draw as a uniform float in [0, 1).
pub fn u32_to_unit(x: u32) -> f64 {
    x as f64 / 4_294_967_296.0
}

#[cfg(test)]
#[path = "rng.test.rs"]
mod test;
