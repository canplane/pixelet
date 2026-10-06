// pxl-spec/space/morton.rs

//! A node's one key: the lanes in offset binary, by their half width added
//! once, interleaved a zoom at a time from the coarsest in `slot_of`'s order.
//! An axis takes part from the zoom its lane reaches: X and Y from z1, Z, 16
//! bits under X's 24, from z9, so a 3D key is the 2D one to z8. A coarser key
//! is a prefix of a finer one over the same ground, in any number of axes.

use super::pvf3::core::{IVEC_X_W, IVEC_Y_W, IVEC_Z_W};
use super::zoom::{Zoom, UNIT_Z};

const LANES: [u32; 3] = [IVEC_X_W, IVEC_Y_W, IVEC_Z_W];

/// Whether axis `axis` has a digit at zoom `zoom` (1 ..= UNIT_Z).
const fn takes_part(axis: usize, zoom: Zoom) -> bool {
    zoom as u32 + LANES[axis] > UNIT_Z as u32
}

/// A key's bits at `zoom`.
pub const fn bits<const D: usize>(zoom: Zoom) -> u32 {
    let (mut n, mut z) = (0, 1);
    while z <= zoom {
        let mut axis = 0;
        while axis < D {
            n += takes_part(axis, z) as u32;
            axis += 1;
        }
        z += 1;
    }
    n
}

/// The key of the node at `zoom` holding the point `at`, in units on each lane.
pub fn key<const D: usize>(zoom: Zoom, at: [i64; D]) -> u64 {
    let lane: [u64; D] = std::array::from_fn(|i| (at[i] + (1 << (LANES[i] - 1))) as u64);
    (1..=zoom).fold(0, |key, z| (0..D).rev().filter(|&i| takes_part(i, z)).fold(key, |key, i| (key << 1) | ((lane[i] >> (UNIT_Z - z)) & 1)))
}

/// The point in units at the node's corner nearest the lanes' least: `key`'s inverse.
pub fn base<const D: usize>(zoom: Zoom, key: u64) -> [i64; D] {
    let mut lane = [0u64; D];
    let mut left = bits::<D>(zoom);
    for z in 1..=zoom {
        for (_, v) in lane.iter_mut().enumerate().rev().filter(|&(i, _)| takes_part(i, z)) {
            left -= 1;
            *v |= ((key >> left) & 1) << (UNIT_Z - z);
        }
    }
    std::array::from_fn(|i| lane[i] as i64 - (1 << (LANES[i] - 1)))
}

const _: () = assert!(IVEC_X_W == UNIT_Z as u32 && IVEC_Y_W == IVEC_X_W, "X and Y digit from z1");
const _: () = assert!(bits::<3>(UNIT_Z) == u64::BITS, "a unit's 3D key is one word");

#[cfg(test)]
#[path = "morton.test.rs"]
mod test;
