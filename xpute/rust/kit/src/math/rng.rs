// xpute-kit/math/rng.rs

//! Reproducible seeds and random streams, not for anything that must resist
//! prediction. There are two families, a file each: a 64-bit one built on
//! splitmix64 (`rng.64.rs`), and a 32-bit one built on Murmur3's finalizer
//! (`rng.32.rs`), which a GPU shader can compute as well.

#[path = "rng.32.rs"]
mod w32;
#[path = "rng.64.rs"]
mod w64;

pub use w32::{derive_seed_u32, mix32, SplitMix32, MIX32_FIRST, MIX32_LAST, SEED_SALT_32};
pub use w64::{derive_seed_u64, mix64, SplitMix64};

/// Maps a `u32` onto [0, 1), uniformly.
pub fn u32_to_unit(x: u32) -> f64 {
    x as f64 / 4_294_967_296.0
}
