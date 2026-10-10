// pxl-spec/space/hierarchy.rs

//! What a level is to the one under it. Every extent is a power of two of the
//! unit, a `Scale`, so sizes from different bases cannot mix in one
//! expression. In position, a parent is the coordinate halved and a child is
//! told apart by each axis's low bit, in any number of axes.

use super::pvf3::core::FRAC_W;
use super::pvf3::pvf3::{self, Pvf3};
use super::v3::Aabb;
use xpute_core::math::scalar::round;

/// A side of 2^shamt units: negative where it is finer than a unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Scale(i32);

impl Scale {
    /// Where a root is spelled; every other scale is stepped from one.
    pub const fn of(shamt: i32) -> Scale {
        Scale(shamt)
    }

    pub const fn shamt(self) -> i32 {
        self.0
    }

    /// Negative `levels` steps down.
    pub const fn up(self, levels: i32) -> Scale {
        Scale(self.0 + levels)
    }

    pub const fn down(self, levels: u32) -> Scale {
        Scale(self.0 - levels as i32)
    }

    /// Negative where below.
    pub const fn above(self, base: Scale) -> i32 {
        self.0 - base.0
    }

    pub const fn count_in(self, outer: Scale) -> u32 {
        if outer.0 < self.0 {
            panic!()
        }
        1 << (outer.0 - self.0)
    }

    pub const fn side(self) -> f64 {
        if self.0 >= 0 {
            (1u64 << self.0) as f64
        } else {
            1.0 / (1u64 << -self.0) as f64
        }
    }

    /// A floor on the signed lane.
    pub const fn index(self, units: i64) -> i64 {
        if self.0 >= 0 {
            units >> self.0
        } else {
            units << -self.0
        }
    }

    /// A floor on each signed lane; exact while a side is no finer than the fraction.
    pub fn index_of(self, at: &Pvf3) -> [i64; 3] {
        let shift = FRAC_W as i32 + self.0;
        [pvf3::X(at) >> shift, pvf3::Y(at) >> shift, pvf3::Z(at) >> shift]
    }

    /// `index_of`'s inverse.
    pub fn base_of(self, index: [i64; 3]) -> Pvf3 {
        let shift = FRAC_W as i32 + self.0;
        pvf3::gather(index[0] << shift, index[1] << shift, index[2] << shift)
    }

    pub fn cube(self, index: [i64; 3]) -> Aabb {
        let side = self.side();
        let min = index.map(|c| c as f64 * side);
        Aabb { min, max: min.map(|v| v + side) }
    }

    pub fn of_side(units: f64) -> Scale {
        Scale(round(libm::log2(units)) as i32)
    }
}

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

pub fn ancestor_of<const D: usize>(at: [i64; D], levels: u32) -> [i64; D] {
    at.map(|v| v >> levels)
}

/// Where `at` lies in its ancestor `levels` up, from that ancestor's first
/// descendant.
pub fn offset_in<const D: usize>(at: [i64; D], levels: u32) -> [i64; D] {
    at.map(|v| v & ((1 << levels) - 1))
}

/// Every slot 0: `ancestor_of`'s inverse.
pub fn first_descendant_of<const D: usize>(at: [i64; D], levels: u32) -> [i64; D] {
    at.map(|v| v << levels)
}

/// Whether two nodes overlap; in one hierarchy they nest or are apart.
pub fn share<const D: usize>((a_side, a): (Scale, [i64; D]), (b_side, b): (Scale, [i64; D])) -> bool {
    if a_side <= b_side {
        ancestor_of(a, b_side.above(a_side) as u32) == b
    } else {
        ancestor_of(b, a_side.above(b_side) as u32) == a
    }
}

/// A node's position in a depth-first walk in slot order, coarsest level
/// first; the leaves of one coarser node form one run.
pub fn walk_order<const D: usize>(at: [u32; D], levels: u32) -> u64 {
    (0..levels).rev().fold(0, |order, l| (order << D) | slot_of(at.map(|v| i64::from(v >> l))) as u64)
}

pub fn at_walk_order<const D: usize>(order: u64, levels: u32) -> [u32; D] {
    (0..levels).rev().fold([0; D], |at, l| {
        let slot = ((order >> (l as usize * D)) & ((1 << D) - 1)) as usize;
        child_of(at.map(i64::from), slot).map(|v| v as u32)
    })
}

#[cfg(test)]
#[path = "hierarchy.test.rs"]
mod test;
