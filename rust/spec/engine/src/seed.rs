// pxl-engine/seed.rs

//! gpu-portable: a node's seed, which every draw on the world's nodes starts
//! from, a u32 hash over i32 cells: the caves', the ground's decor's and a
//! placed decor's pose.

use xpute_kit::math::rng::derive_seed_u32;

use crate::{wrap_idx, Zoom};

/// The seed every draw on the world's nodes starts from (`node_seed`): the
/// caves, the ground's decor and a placed decor's pose.
const MASTER_SEED: u32 = 0x5eed_0001;

/// A node's seed for one use: `tag` names the use, and the node's zoom and
/// each of its indices are folded in, in order, X named as the world names
/// it (`wrap_idx`), so a node seeds alike on whichever lap its asker counts
/// it. Every draw on the world's nodes seeds by it, each use under a tag of
/// its own, so no two uses draw one stream.
pub fn node_seed<const D: usize>(tag: u32, (zoom, at): (Zoom, [i32; D])) -> u32 {
    const { assert!(D <= 3, "a node has at most three indices") };
    let mut h = derive_seed_u32(derive_seed_u32(MASTER_SEED, tag), zoom);
    // [naga] each index by itself, not a loop: Naga's Metal backend breaks a
    // `loop { … continuing { break if … } }` an iteration early, re-reading
    // its condition after the counter moved.
    if D > 0 {
        h = derive_seed_u32(h, wrap_idx(zoom, at[0]) as u32);
    }
    if D > 1 {
        h = derive_seed_u32(h, at[1] as u32);
    }
    if D > 2 {
        h = derive_seed_u32(h, at[2] as u32);
    }
    h
}
