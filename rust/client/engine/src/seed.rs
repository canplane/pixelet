// pxl-engine/seed.rs

//! wgsl-port: a node's seed, the u32 hash of a node's zoom and cells from which every
//! random draw over the world's nodes starts: the caves, the ground's decor, and a placed
//! decor's pose.

use xpute_kit::math::rng::seed_mix32;

use pxl_spec::space::coord::world::wrap_idx;
use pxl_spec::space::coord::zoom::Zoom;

/// The root of every node's seed (`node_seed`).
pub const MASTER_SEED: u32 = 0x5eed_0001;

/// A node's seed for one use, named by `tag`. The node's zoom and its indices are folded in,
/// in order. X is first wrapped to the world's own index (`wrap_idx`), so a node gets the
/// same seed on whichever lap around the world its asker counts it. Each use has a tag of its
/// own, so no two uses draw from the same stream; a use's several draws of one node fold their
/// index onto its seed (`seed_mix32`).
pub fn node_seed<const D: usize>(tag: u32, (zoom, at): (Zoom, [i32; D])) -> u32 {
    const { assert!(D <= 3, "a node has at most three indices") };
    let mut h = seed_mix32(seed_mix32(MASTER_SEED, tag), zoom);
    // [naga] Each index is folded by itself, not in a loop: Naga's Metal backend leaves a
    // `loop { … continuing { break if … } }` an iteration early, because it re-reads the
    // condition after the counter has moved.
    if D > 0 {
        h = seed_mix32(h, wrap_idx(zoom, at[0]) as u32);
    }
    if D > 1 {
        h = seed_mix32(h, at[1] as u32);
    }
    if D > 2 {
        h = seed_mix32(h, at[2] as u32);
    }
    h
}
