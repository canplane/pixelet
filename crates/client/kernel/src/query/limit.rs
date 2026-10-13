// pxl-kernel/query/limit.rs

//! Where the world ends for what moves in it: the service area, the shards
//! its source serves (io/source.rs), which a walker does not leave, and where
//! a block may be placed in or taken from, over ground the world knows
//! between its bedrock and its ceiling. Every walk and every edit asks these,
//! and nothing else decides them.

use pxl_spec::block::cube::sentinel;
use pxl_spec::block::limit::{at_edge, ceiling, floor};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::world::point;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use super::World;
use crate::io::source;
use pxl_spec::space::coord::q3::{self, Q3};

impl World<'_> {
    /// Whether the leaf cell under `at` is the service area's and off its
    /// edge (`pxl-spec block/limit.rs` `at_edge`), where the wall stands:
    /// known before any of its ground arrives.
    pub fn within(&self, at: &Q3) -> bool {
        let [x, y] = [i64::from(at[0]), i64::from(at[1])].map(q3::to_real);
        let leaf = units(GROUND.cell());
        !at_edge(|dx, dy| source::past_at([x + dx as f64 * leaf, y + dy as f64 * leaf]))
    }

    /// Whether `space` may hold an edit: every leaf cell under it is the
    /// service area's, and there `space` lies over the bedrock and under the
    /// ceiling of ground the world knows exactly, no tile not yet held
    /// (`pxl-spec block/limit.rs`), and no cell is bedrock, the wall: what
    /// is `unknown` is neither placed on nor taken.
    pub fn editable(&self, space: &Aabb) -> bool {
        let unit = units(GROUND.cell());
        let cells = |k: usize| (space.min[k] / unit).floor() as i64..(space.max[k] / unit).ceil() as i64;
        cells(1).all(|y| {
            cells(0).all(|x| {
                let center = point((x as f64 + 0.5) * unit, (y as f64 + 0.5) * unit, space.min[2]);
                // The shards first, which need no ground; then the limits of the
                // ground there, not its runs: a fold draws no bedrock.
                self.within(&center)
                    && self
                        .surface(&center, LEAF_ZOOM)
                        .exact()
                        .flatten()
                        .is_some_and(|s| s.block != sentinel::unknown && floor(s.top) <= space.min[2] && space.max[2] <= ceiling(s.top))
            })
        })
    }
}
