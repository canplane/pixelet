// pxl-spec/space/hierarchy.rs

//! What a level is to the one under it, in size and in position.
//!
//! In size, every extent in this world is a power of two of the unit: the
//! tiers (indexer.rs), a lattice's cell and each level over it (pyramid.rs),
//! an object's tiers and the band its tree stands in. A `Scale` is that
//! power, and an extent is compared, stepped and counted only as one, so two
//! numbers measured from different bases — a pyramid level counted from a
//! lattice's cell and a size counted from the unit — cannot meet in one
//! expression. Where a level is said as a count, the lattice it counts from
//! turns it into a Scale.
//!
//! In position, what a level is to the one under it does not depend on how
//! many axes there are: a node's parent is its coordinate halved, and it is
//! one of its parent's 2^D children, told apart by the low bit of each axis.
//! The ground's pages and cells are this in two (pyramid.rs `LevelPage`,
//! `Cell`), an object's cells in three (pxl-render mesh/paint/object/fold.rs),
//! a sparse tree's blocks in either (sparse_tree/mem.rs), and a fold reads
//! the children in this order whichever it is (bake/fold.rs `majority`).

use super::pvf3::core::FRAC_W;
use super::pvf3::pvf3::{self, Pvf3};

/// A side of 2^shamt units: negative where it is finer than a unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Scale(i32);

impl Scale {
    /// The scale whose side is 2^shamt units. Where a root is spelled; every
    /// other scale is stepped from one.
    pub const fn of(shamt: i32) -> Scale {
        Scale(shamt)
    }

    /// The side as a power of two of the unit.
    pub const fn shamt(self) -> i32 {
        self.0
    }

    /// The scale `levels` above, or below where negative.
    pub const fn up(self, levels: i32) -> Scale {
        Scale(self.0 + levels)
    }

    /// The scale `levels` below.
    pub const fn down(self, levels: u32) -> Scale {
        Scale(self.0 - levels as i32)
    }

    /// How many levels this is above `base`: negative where it is below.
    pub const fn above(self, base: Scale) -> i32 {
        self.0 - base.0
    }

    /// How many of this scale's sides lie along one of `outer`'s.
    pub const fn count_in(self, outer: Scale) -> u32 {
        assert!(outer.0 >= self.0, "no coarser than what it is counted in");
        1 << (outer.0 - self.0)
    }

    /// The side in units.
    pub const fn side(self) -> f64 {
        if self.0 >= 0 {
            (1u64 << self.0) as f64
        } else {
            1.0 / (1u64 << -self.0) as f64
        }
    }

    /// Which of this scale's sides along an axis a whole number of units
    /// lies in, counted from the origin: a floor on the signed lane.
    pub const fn index(self, units: i64) -> i64 {
        if self.0 >= 0 {
            units >> self.0
        } else {
            units << -self.0
        }
    }

    /// Which of this scale's sides a point lies in on each axis, counted from
    /// the origin: a floor on the fixed point's signed lanes, exact while a
    /// side is no finer than the fraction.
    pub fn index_of(self, at: &Pvf3) -> [i64; 3] {
        let shift = FRAC_W as i32 + self.0;
        [pvf3::X(at) >> shift, pvf3::Y(at) >> shift, pvf3::Z(at) >> shift]
    }

    /// The corner of the side at `index` on each axis: `index_of`'s inverse.
    pub fn base_of(self, index: [i64; 3]) -> Pvf3 {
        let shift = FRAC_W as i32 + self.0;
        let mut out = Pvf3::default();
        pvf3::gather(&mut out, index[0] << shift, index[1] << shift, index[2] << shift);
        out
    }

    /// The scale whose side is nearest `units`, a power of two where it is
    /// one.
    pub fn of_side(units: f64) -> Scale {
        Scale(units.log2().round() as i32)
    }
}

/// The node a level up that `at` folds into: every axis halved, a floor on
/// the signed lanes.
pub fn parent_of<const D: usize>(at: [i64; D]) -> [i64; D] {
    at.map(|v| v.div_euclid(2))
}

/// Which of its parent's 2^D children `at` is: axis i's low bit at bit i,
/// `x + 2y` in two dimensions and `x + 2y + 4z` in three.
pub fn slot_of<const D: usize>(at: [i64; D]) -> usize {
    (0..D).map(|i| (at[i].rem_euclid(2) as usize) << i).sum()
}

/// The child in `slot` of the node at `at`: `parent_of` and `slot_of`'s
/// inverse.
pub fn child_of<const D: usize>(at: [i64; D], slot: usize) -> [i64; D] {
    std::array::from_fn(|i| 2 * at[i] + ((slot >> i) & 1) as i64)
}

/// The node `levels` up that `at` lies under: `parent_of` taken `levels`
/// times, which on the signed lanes is one floor.
pub fn ancestor_of<const D: usize>(at: [i64; D], levels: u32) -> [i64; D] {
    at.map(|v| v >> levels)
}

/// The first of the nodes `levels` down under `at`, every slot 0: `child_of`
/// taken `levels` times, and `ancestor_of`'s inverse there.
pub fn first_descendant_of<const D: usize>(at: [i64; D], levels: u32) -> [i64; D] {
    at.map(|v| v << levels)
}

/// Where a depth-first walk that visits each node's children in slot order
/// reaches the node at `at`, `levels` below the block it starts from: each
/// level's slot (`slot_of`), the coarsest first. Nodes a walk visits
/// together — the leaves of one coarser node — are one run of this order.
pub fn walk_order<const D: usize>(at: [u32; D], levels: u32) -> u64 {
    (0..levels).rev().fold(0, |order, l| (order << D) | slot_of(at.map(|v| i64::from(v >> l))) as u64)
}

/// The node `walk_order` puts at `order`: its inverse.
pub fn at_walk_order<const D: usize>(order: u64, levels: u32) -> [u32; D] {
    (0..levels).rev().fold([0; D], |at, l| {
        let slot = ((order >> (l as usize * D)) & ((1 << D) - 1)) as usize;
        child_of(at.map(i64::from), slot).map(|v| v as u32)
    })
}

#[cfg(test)]
#[path = "hierarchy.test.rs"]
mod test;
