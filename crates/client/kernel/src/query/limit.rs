// pxl-kernel/query/limit.rs

//! Where the world ends for what moves in it: the service area, the shards
//! its source serves (io/source.rs), which a walker does not leave, and where
//! a block may be placed in or taken from, over ground the world knows
//! between its bedrock and its ceiling. Every walk and every edit asks these,
//! and nothing else decides them.

use pxl_spec::block::limit::{ceiling, floor};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::world::point;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use super::World;
use crate::io::source;
use pxl_spec::space::coord::pvf3::pvf3::{self, Pvf3};

impl World<'_> {
    /// Whether the leaf column under `at` is the service area's, known
    /// before any of its ground arrives.
    pub fn within(&self, at: &Pvf3) -> bool {
        !source::past_at([pvf3::X(at), pvf3::Y(at)].map(pvf3::to_real))
    }

    /// Whether `space` may hold an edit: every leaf column under it is the
    /// service area's, and there `space` lies over the bedrock and under the
    /// ceiling of ground the world knows exactly, no tile not yet held
    /// (`pxl-spec block/limit.rs`).
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
                        .runs_at(&center, LEAF_ZOOM, |_, surface| surface.is_some_and(|s| floor(s.top) <= space.min[2] && space.max[2] <= ceiling(s.top)))
                        .exact()
                        == Some(true)
            })
        })
    }
}
