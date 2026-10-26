// xpute-kit/math/rng.rs

//! Reproducible seeds and streams; not for anything that must resist prediction.
//! Two families, a file each: 64-bit (splitmix64, `rng.64.rs`) and 32-bit
//! (Murmur3's finalizer, `rng.32.rs`), the latter gpu-portable, since WebGPU
//! has no 64-bit integer.

#[path = "rng.32.rs"]
mod w32;
#[path = "rng.64.rs"]
mod w64;

pub use w32::{derive_seed_u32, mix32, SplitMix32};
pub use w64::{derive_seed_u64, mix64, SplitMix64};

/// Uniform in [0, 1).
pub fn u32_to_unit(x: u32) -> f64 {
    x as f64 / 4_294_967_296.0
}
