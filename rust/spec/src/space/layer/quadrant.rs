// pxl-spec/space/layer/quadrant.rs

//! The pyramid: tiles of a tile grid from the leaves up to the top, a tile's zoom given by its
//! size, so 2^z of them stand around the world. Tile membership and folding order are decided
//! here only; a layer owns what a cell carries and how it folds.

use core::fmt;

use crate::space::coord::aabb::Aabb;
use crate::space::coord::q3;
use crate::space::coord::world::{base_of, wrap_idx};
use crate::space::coord::zoom::{LEAF_ZOOM, MIN_ZOOM, Zoom, units};
use crate::space::hierarchy::morton::{self, key_of, node_of};
use crate::space::hierarchy::tree::{Node, ancestor_of, child_idx, child_of, parent_of, region};
use crate::space::layer::area::shard_over;

const _: () = assert!(
    morton::bits::<2>(LEAF_ZOOM) < 64,
    "a quadrant's id fits a u64"
);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Quadrant {
    /// Its zoom, which is its size.
    pub zoom: Zoom,
    /// Its index at its zoom.
    pub at: [i32; 2],
}

impl Quadrant {
    /// The node of `zoom` at `at`, named as `wrap_idx` names its X, so one has
    /// one name.
    pub fn new(zoom: Zoom, [x, y]: [i32; 2]) -> Quadrant {
        Quadrant {
            zoom,
            at: [wrap_idx(zoom, x), y],
        }
    }

    /// The tile at `zoom` the leaf `leaf` lies under.
    pub fn over(zoom: Zoom, leaf: [i32; 2]) -> Quadrant {
        Quadrant::new(zoom, ancestor_of(leaf, LEAF_ZOOM - zoom))
    }

    #[inline]
    pub fn parent(self) -> Quadrant {
        Quadrant::new(self.zoom - 1, parent_of(self.at))
    }

    /// Its 2D morton code at a 3D one's width, X wrapped at the seam (`morton::key_of`).
    fn bits(self) -> u128 {
        let [x, y] = self.at;
        key_of::<2>(self.zoom, [wrap_idx(self.zoom, x), y])
    }

    /// Its key, its quadkey as text: a digit a zoom, coarsest first, what a baked
    /// tile is stored as, on disk and in the bucket alike, and what the core
    /// opens it by.
    pub fn key(self) -> String {
        morton::name::<2>(self.bits())
    }

    /// `key`, written where it is wanted.
    pub fn write_key(self, out: &mut impl fmt::Write) -> fmt::Result {
        morton::write_name::<2>(out, self.bits())
    }

    pub fn of_key(key: &str) -> Option<Quadrant> {
        let (zoom, at) = node_of::<2>(morton::key_of_name::<2>(key)?);
        (MIN_ZOOM..=LEAF_ZOOM)
            .contains(&zoom)
            .then(|| Quadrant::new(zoom, at))
    }

    /// The shard over it, or itself where it is a shard or coarser, which no one
    /// shard is over (`area::shard_over`).
    pub fn shard(self) -> Quadrant {
        let (zoom, at) = shard_over(self.zoom, self.at);
        Quadrant::new(zoom, at)
    }

    /// Its place among its parent's `children`.
    pub fn slot(self) -> usize {
        child_idx(self.at)
    }

    /// Its four children, in `child_idx`'s order.
    pub fn children(self) -> [Quadrant; 4] {
        std::array::from_fn(|slot| Quadrant::new(self.zoom + 1, child_of(self.at, slot)))
    }

    pub fn node(self) -> Node<2> {
        (self.zoom, self.at)
    }

    /// South-west corner and side, in units.
    pub fn rect(self) -> (f64, f64, f64) {
        let (zoom, [x, y]) = self.node();
        let base = base_of(zoom, [x, y, 0]);
        (
            q3::to_real(i64::from(base[0])),
            q3::to_real(i64::from(base[1])),
            units(zoom),
        )
    }

    /// The tile as a box of the world: its square, every height.
    pub fn region(self) -> Aabb {
        region(self.node())
    }

    /// Its id, its morton code as a number (`morton.rs`): its digits from the top,
    /// then the 1 that ends them, all in a word.
    pub fn id(self) -> u64 {
        (self.bits() >> 64) as u64
    }

    pub fn of_id(id: u64) -> Quadrant {
        let (zoom, at) = node_of::<2>(u128::from(id) << 64);
        Quadrant::new(zoom, at)
    }
}

/// The coarsest tile of a layer made at `leaf` when the source supplies
/// terrain whole from `min_zoom`: as many zooms over its leaves.
pub fn min_zoom_over(leaf: Zoom, min_zoom: Zoom) -> Zoom {
    min_zoom.saturating_sub(LEAF_ZOOM - leaf).max(MIN_ZOOM)
}

/// The 4×4 tiles a zoom finer around a tile's children, each at
/// `neighbor_idx`; X wraps and a tile past Y's edge has no object.
pub fn neighborhood(p: Quadrant) -> [Quadrant; 16] {
    let [x, y] = child_of(p.at, 0);
    std::array::from_fn(|k| {
        let (qx, qy) = ((k % 4) as i32 - 1, (k / 4) as i32 - 1);
        Quadrant::new(p.zoom + 1, [x + qx, y + qy])
    })
}

/// Where the tile (qx, qy) from a parent's first child, each −1 to 2, lies in
/// its `neighborhood`.
pub(crate) const fn neighbor_idx(qx: i32, qy: i32) -> usize {
    ((qx + 1) + 4 * (qy + 1)) as usize
}

/// The rows of tiles at `zoom`, south to north; past them is no tile.
pub fn quad_rows(zoom: Zoom) -> std::ops::Range<i32> {
    let n = 1 << zoom;
    -n / 2..n / 2
}

/// Where a tile's own children sit in its neighborhood, in `child_idx`'s order.
pub const CHILDREN: [usize; 4] = [5, 6, 9, 10];

#[cfg(test)]
#[path = "quadrant.test.rs"]
mod test;
