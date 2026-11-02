// pxl-kernel/domain/query/limit.rs

//! Where the world ends for what moves in it: the service area, the shards
//! its source serves (sys/io/source.rs), which a walker does not leave, and where
//! an asset may be placed or taken out, over ground the world knows
//! between its bedrock and its ceiling, in shards whose objects are read.
//! Every walker's walk and every edit
//! asks these, and nothing else decides them; the camera is no walker and
//! asks none (query/support.rs `step_slide`).

use pxl_spec::limit::{bedrock, ceiling};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::world::point;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::{Cells, GROUND};
use pxl_spec::space::layer::tile::LEAF_ZOOM;
use pxl_spec_core::area::{is_wall, walls_in, Area, Shard};
use pxl_spec_core::wrap_idx;

use super::World;
use crate::domain::layer::object::repository;
use crate::domain::layer::terrain::TerrainQuery as _;
use crate::domain::layer::Query;
use crate::sys::io::source;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec_core::clipmap::Lookup;

impl World<'_> {
    /// Whether the leaf column under `at` is the service area's and not the
    /// wall (pxl-spec-core `area::is_wall`), known before any of its ground
    /// arrives, off its shard's nine, the area's index asked once
    /// (`Shard::at`). A walker comes to the wall's face and stands on none
    /// of it.
    pub fn within(&self, at: &Q3) -> bool {
        within_in(&source::ServiceArea, at)
    }

    /// Whether `space` may hold an edit: every shard under it read, so a
    /// write stands over what its file held (domain/layer/object/repository.rs
    /// `holds`); none of its leaf columns the wall (pxl-spec-core
    /// `area::walls_in`), as `within` holds; and there `space` over the
    /// bedrock and under the ceiling of ground the world knows exactly, no
    /// tile not yet held (`pxl-spec limit.rs`).
    /// The bedrock and the ceiling rise with the face, so every column's
    /// hold where the lowest ceiling and the highest bedrock do: the
    /// ground's bound over the nodes holding `space` at its leaf cells, the
    /// fewest at the zoom of its widest side, two a side at most
    /// (`TileCache::relief`). An aligned cube is one node, its own columns;
    /// a box that is not is bounded by the nodes about it, which only
    /// refuses more.
    pub fn editable(&self, space: &Aabb) -> bool {
        repository::holds(space) && self.editable_in(&source::ServiceArea, space)
    }

    /// `editable` against the service area `area`, the shards aside.
    pub(super) fn editable_in(&self, area: &impl Area, space: &Aabb) -> bool {
        let (cell, unit) = (GROUND.cell(), units(GROUND.cell()));
        let first = |k: usize| (space.min[k] / unit).floor() as i32;
        let cnt = |k: usize| ((space.max[k] / unit).ceil() as i32 - first(k)).max(1);
        let last = |k: usize| first(k) + cnt(k) - 1;
        if walls_in(area, [[first(0), first(1)], [last(0), last(1)]]) {
            return false;
        }
        let s = (cnt(0).max(cnt(1)) as u64)
            .next_power_of_two()
            .trailing_zeros();
        let zoom = cell - s;
        let nodes = |k: usize| first(k) >> s..=(first(k) + cnt(k) - 1) >> s;
        nodes(1).all(|y| {
            nodes(0).all(|x| {
                let node = (zoom, [wrap_idx(zoom, x), y]);
                let (line, mut place) = (self.terrain.cursor(node, cell), Lookup::NONE);
                // The ground's faces, in leaf cells.
                self.terrain
                    .relief(&line, &mut place, node, [cell, cell])
                    .exact()
                    .is_some_and(|[[lo, hi], _]| {
                        f64::from(bedrock(hi)) * unit <= space.min[2]
                            && space.max[2] <= f64::from(ceiling(lo)) * unit
                    })
            })
        })
    }
}

/// `World::within` against the service area `area`.
pub(super) fn within_in(area: &impl Area, at: &Q3) -> bool {
    let [x, y] = [i64::from(at[0]), i64::from(at[1])].map(q3::to_real);
    let (cell, b) = GROUND.cell_of(LEAF_ZOOM, &point(x, y, 0.0)).node();
    !is_wall(Shard::UNKNOWN.at(area, (cell, b)), cell, b)
}
