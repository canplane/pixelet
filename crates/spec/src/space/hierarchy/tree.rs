// pxl-spec/space/hierarchy/tree.rs

//! What a node is to the one it lies in. In position, a parent is the
//! coordinate halved and a child is told apart by each axis's low bit, in any
//! number of axes; a node's size is its zoom (`zoom.rs`).

use crate::space::coord::zoom::Zoom;

/// A floor on the signed lanes.
pub fn parent_of<const D: usize>(at: [i64; D]) -> [i64; D] {
    at.map(|v| v.div_euclid(2))
}

/// How many slots `slot_of` numbers in `axes` axes.
pub const fn children(axes: usize) -> usize {
    1 << axes
}

/// Axis i's low bit at bit i: `x + 2y + 4z`.
pub fn slot_of<const D: usize>(at: [i64; D]) -> usize {
    (0..D).map(|i| (at[i].rem_euclid(2) as usize) << i).sum()
}

/// `parent_of` and `slot_of`'s inverse.
pub fn child_of<const D: usize>(at: [i64; D], slot: usize) -> [i64; D] {
    std::array::from_fn(|i| 2 * at[i] + ((slot >> i) & 1) as i64)
}

pub fn ancestor_of<const D: usize>(at: [i64; D], steps: u32) -> [i64; D] {
    at.map(|v| v >> steps)
}

/// Where `at` lies in its ancestor `steps` up, from that ancestor's first
/// descendant.
pub fn offset_in<const D: usize>(at: [i64; D], steps: u32) -> [i64; D] {
    at.map(|v| v & ((1 << steps) - 1))
}

/// Every slot 0: `ancestor_of`'s inverse.
pub fn first_descendant_of<const D: usize>(at: [i64; D], steps: u32) -> [i64; D] {
    at.map(|v| v << steps)
}

/// Whether two nodes overlap; in one hierarchy they nest or are apart.
pub fn share<const D: usize>((a_z, a): (Zoom, [i64; D]), (b_z, b): (Zoom, [i64; D])) -> bool {
    if a_z >= b_z {
        ancestor_of(a, u32::from(a_z - b_z)) == b
    } else {
        ancestor_of(b, u32::from(b_z - a_z)) == a
    }
}

/// A node's position in a depth-first walk in slot order, coarsest step
/// first; the leaves of one coarser node form one run.
pub fn walk_order<const D: usize>(at: [u32; D], steps: u32) -> u64 {
    (0..steps).rev().fold(0, |order, l| (order << D) | slot_of(at.map(|v| i64::from(v >> l))) as u64)
}

pub fn at_walk_order<const D: usize>(order: u64, steps: u32) -> [u32; D] {
    (0..steps).rev().fold([0; D], |at, l| {
        let slot = ((order >> (l as usize * D)) & ((1 << D) - 1)) as usize;
        child_of(at.map(i64::from), slot).map(|v| v as u32)
    })
}

#[cfg(test)]
#[path = "tree.test.rs"]
mod test;
