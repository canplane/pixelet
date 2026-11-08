// xpute-kit/math/rng.64.rs

//! The 64-bit family: splitmix64's finalizer, a seed derived through it,
//! and splitmix64's stream, drawn as `u32` from the high 32 bits of each
//! word.

const SEED_SALT: u64 = 0x9e3779b97f4a7c15;

/// splitmix64's finalizer applied to `x` plus the golden-ratio salt, which
/// is one splitmix64 step from the state `x`.
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

/// splitmix64's stream, drawn as `u32`: each draw is the high 32 bits of
/// the next 64-bit output.
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
#[path = "rng.64.test.rs"]
mod test;
