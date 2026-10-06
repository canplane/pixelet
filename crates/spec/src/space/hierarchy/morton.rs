// pxl-spec/space/hierarchy/morton.rs

//! A node's one key at any zoom: its index's digits in two's complement, as
//! the signed indices are, interleaved a zoom at a time from the coarsest in
//! `slot_of`'s order, left-aligned, then a 1 that marks where they end. An
//! axis has a digit from the zoom its lane reaches: X and Y from z1, Z, 16
//! bits under X's 24, from z9, so a 3D key is the 2D one to z8 and an axis's
//! first digit is its sign. A coarser key's digits prefix a finer one's over
//! the same ground, in any number of axes.

use crate::space::coord::pvf3::core::{IVEC_X_W, IVEC_Y_W, IVEC_Z_W};
use crate::space::coord::zoom::{Zoom, UNIT_ZOOM};

const LANES: [u32; 3] = [IVEC_X_W, IVEC_Y_W, IVEC_Z_W];

/// The digits axis `axis` has at `zoom`: one a zoom from where its lane reaches.
const fn digits(axis: usize, zoom: Zoom) -> u32 {
    (zoom as u32).saturating_sub(UNIT_ZOOM as u32 - LANES[axis])
}

/// A key's digits at `zoom`.
pub const fn bits<const D: usize>(zoom: Zoom) -> u32 {
    let (mut n, mut axis) = (0, 0);
    while axis < D {
        n += digits(axis, zoom);
        axis += 1;
    }
    n
}

/// The axes with a digit at `zoom`, the order they are read in.
fn axes<const D: usize>(zoom: Zoom) -> impl Iterator<Item = usize> {
    (0..D).rev().filter(move |&i| digits(i, zoom) > digits(i, zoom - 1))
}

/// The key of the node at `zoom` whose index is `at`.
pub fn key_of<const D: usize>(zoom: Zoom, at: [i64; D]) -> u128 {
    let n = bits::<D>(zoom);
    let digits = (1..=zoom).fold(0u128, |key, z| axes::<D>(z).fold(key, |key, i| (key << 1) | ((at[i] >> (zoom - z)) & 1) as u128));
    (digits << (127 - n)) | (1 << (126 - n))
}

/// `key_of`'s inverse: the zoom, and each axis's index at it, signed.
pub fn node_of<const D: usize>(key: u128) -> (Zoom, [i64; D]) {
    let n = 126 - key.trailing_zeros();
    let (mut zoom, mut read, mut at) = (0, 0, [0i64; D]);
    while read < n {
        zoom += 1;
        for i in axes::<D>(zoom) {
            read += 1;
            at[i] = (at[i] << 1) | ((key >> (127 - read)) & 1) as i64;
        }
    }
    (zoom, std::array::from_fn(|i| signed(at[i], digits(i, zoom))))
}

/// `v`'s low `width` bits as a two's complement number.
fn signed(v: i64, width: u32) -> i64 {
    match width {
        0 => 0,
        w => (v << (64 - w)) >> (64 - w),
    }
}

const _: () = assert!(IVEC_X_W == UNIT_ZOOM as u32 && IVEC_Y_W == IVEC_X_W, "X and Y digit from z1");

#[cfg(test)]
#[path = "morton.test.rs"]
mod test;
