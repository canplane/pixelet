// pxl-spec/space/hierarchy/tree.rs

//! What a node is to the one it lies in. In position, a parent is the
//! coordinate halved and a child is told apart by each axis's low bit, in any
//! number of axes; a node's size is its zoom (`zoom.rs`).

use crate::space::coord::aabb::Aabb;
use crate::space::coord::world::REACH;
use crate::space::coord::zoom::{units, Zoom};

/// A node of the hierarchy in `D` axes: its zoom and its index there.
pub type Node<const D: usize> = (Zoom, [i32; D]);

/// The box a node stands for: a cube in three axes, and in fewer the column
/// over every axis it does not name, as far as the world reaches.
pub fn region<const D: usize>((zoom, at): Node<D>) -> Aabb {
    let side = units(zoom);
    let mut b = REACH;
    for (k, &v) in at.iter().enumerate() {
        b.min[k] = f64::from(v) * side;
        b.max[k] = b.min[k] + side;
    }
    b
}

/// A floor on the signed axes.
pub fn parent_of<const D: usize>(at: [i32; D]) -> [i32; D] {
    at.map(|v| v.div_euclid(2))
}

/// How many slots `slot_of` numbers in `axes` axes.
pub const fn children(axes: usize) -> usize {
    1 << axes
}

/// Axis i's low bit at bit i: `x + 2y + 4z`.
pub fn slot_of<const D: usize>(at: [i32; D]) -> usize {
    (0..D).map(|i| (at[i].rem_euclid(2) as usize) << i).sum()
}

/// `parent_of` and `slot_of`'s inverse.
pub fn child_of<const D: usize>(at: [i32; D], slot: usize) -> [i32; D] {
    std::array::from_fn(|i| 2 * at[i] + ((slot >> i) & 1) as i32)
}

pub fn ancestor_of<const D: usize>(at: [i32; D], steps: u32) -> [i32; D] {
    at.map(|v| v >> steps)
}

/// Where `at` lies in its ancestor `steps` up, from that ancestor's first
/// descendant.
pub fn off_in<const D: usize>(at: [i32; D], steps: u32) -> [i32; D] {
    at.map(|v| v & ((1 << steps) - 1))
}

/// Every slot 0: `ancestor_of`'s inverse.
pub fn first_descendant_of<const D: usize>(at: [i32; D], steps: u32) -> [i32; D] {
    at.map(|v| v << steps)
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
