// xpute-kit/math/rng.gpu.rs
//
// rng.rs's seeds in the form rust-gpu and WGSL take, kept beside it so the
// two can be compared; `core` alone. Each change from rng.rs says why:
// [wgsl] WebGPU has no 64-bit integer.
//
// gpu-portable: one source for the CPU and, through rust-gpu, WGSL.

/// [wgsl] The golden ratio's 32-bit word, in place of `SEED_SALT`'s 64.
const SEED_SALT: u32 = 0x9e37_79b9;

/// [wgsl] Murmur3's 32-bit finalizer, in place of splitmix64's (`mix64`).
pub fn mix32(x: u32) -> u32 {
    let x = x.wrapping_add(SEED_SALT);
    let mut z = x;
    z = (z ^ (z >> 16)).wrapping_mul(0x85eb_ca6b);
    z = (z ^ (z >> 13)).wrapping_mul(0xc2b2_ae35);
    z ^ (z >> 16)
}

/// [wgsl] u32, as `mix32`.
pub fn derive_seed_u32(base: u32, tag: u32) -> u32 {
    mix32(base ^ tag)
}
