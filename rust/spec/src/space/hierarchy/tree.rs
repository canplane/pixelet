// pxl-spec/space/hierarchy/tree.rs

//! What a node is to the one it lies in. In position, a parent is the
//! coordinate halved and a child is told apart by each axis's low bit, in any
//! number of axes; a node's size is its zoom (`zoom.rs`).

use crate::space::coord::aabb::Aabb;
use crate::space::coord::world::{has_digit, BOUNDS};
use crate::space::coord::zoom::{units, Zoom};

/// A node of the hierarchy in `D` axes: its zoom and its index there.
pub type Node<const D: usize> = (Zoom, [i32; D]);

/// The box a node stands for: a cube in three axes, and in fewer the column
/// over every axis it does not name or has no digit of, as far as the world
/// reaches.
pub fn region<const D: usize>((zoom, at): Node<D>) -> Aabb {
    let side = units(zoom);
    let mut b = BOUNDS;
    for (k, &v) in at.iter().enumerate() {
        if has_digit(k, zoom) {
            b.min[k] = f64::from(v) * side;
            b.max[k] = b.min[k] + side;
        }
    }
    b
}

/// The parent's index: each axis halved, floored on the signed axes.
pub(crate) fn parent_of<const D: usize>(at: [i32; D]) -> [i32; D] {
    at.map(|v| v.div_euclid(2))
}

/// How many children `child_idx` numbers in `axes` axes.
pub const fn children(axes: usize) -> usize {
    1 << axes
}

/// Which of its parent's children `at` is, its orthant (a quadrant in 2D, an octant
/// in 3D): axis i's low bit at bit i, `x + 2y + 4z`.
pub fn child_idx<const D: usize>(at: [i32; D]) -> usize {
    (0..D).map(|i| (at[i].rem_euclid(2) as usize) << i).sum()
}

/// `parent_of` and `child_idx`'s inverse.
pub fn child_of<const D: usize>(at: [i32; D], idx: usize) -> [i32; D] {
    std::array::from_fn(|i| 2 * at[i] + ((idx >> i) & 1) as i32)
}

pub fn ancestor_of<const D: usize>(at: [i32; D], step_cnt: u32) -> [i32; D] {
    at.map(|v| v >> step_cnt)
}

/// Where `at` lies in its ancestor `step_cnt` up, from that ancestor's first
/// descendant.
pub(crate) fn off_in<const D: usize>(at: [i32; D], step_cnt: u32) -> [i32; D] {
    at.map(|v| v & ((1 << step_cnt) - 1))
}

/// The descendant `step_cnt` down that takes slot 0 at every step: the inverse of
/// `ancestor_of`.
pub fn first_descendant_of<const D: usize>(at: [i32; D], step_cnt: u32) -> [i32; D] {
    at.map(|v| v << step_cnt)
}

/// Whether two nodes overlap; in one hierarchy they nest or are apart.
pub fn share<const D: usize>((a_zoom, a): Node<D>, (b_zoom, b): Node<D>) -> bool {
    if a_zoom >= b_zoom {
        ancestor_of(a, a_zoom - b_zoom) == b
    } else {
        ancestor_of(b, b_zoom - a_zoom) == a
    }
}

#[cfg(test)]
#[path = "tree.test.rs"]
mod test;
