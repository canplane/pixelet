// pxl-core/domain/query/limit.rs

//! Where the world ends for what moves in it. A walker does not leave the
//! service area, the shards the source serves (sys/io/source.rs). An asset
//! may be placed or taken out only over ground the world knows, between its
//! bedrock and its ceiling, in shards whose objects are read. Every walker's
//! walk and every edit asks these limits, and nothing else decides them. The
//! camera is not a walker and asks none of them (query/support.rs
//! `step_slide`). The UI is told whether the pivot is past the area (`past`).

use pxl_engine::area::{self, is_wall, walls_in, Area, Shard};
use pxl_engine::wrap_idx;
use pxl_spec::limit::{bedrock, ceiling};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::world::point;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::{Grid, GROUND};
use pxl_spec::space::layer::quadrant::LEAF_ZOOM;

use super::World;
use crate::domain::layer::object::repository;
use crate::domain::layer::terrain::TerrainQuery as _;
use crate::domain::layer::Query;
use crate::sys::io::source;
use pxl_engine::clipmap::Lookup;
use pxl_spec::space::coord::q3::{self, Q3};

impl World<'_> {
    /// Whether the leaf column under `at` lies in the service area and is
    /// not the wall at its edge (pxl-engine `area::is_wall`). The answer is
    /// known before any of the column's ground arrives: it comes from the
    /// nine shards around the column's own, with the area's index asked once
    /// (`Shard::at`). A walker comes up to the wall's face and never stands
    /// on the wall.
    pub fn within(&self, at: &Q3) -> bool {
        within_in(&source::ServiceArea, at)
    }

    /// Whether the leaf tile under `at` is past the service area: the source
    /// is declared and serves no shard over it (pxl-engine `area::past`).
    /// The UI is told this of the pivot (ui.rs `outside`). A walk or an edit
    /// asks `within` instead, which also keeps off the area's edge.
    pub fn past(&self, at: &Q3) -> bool {
        let [x, y] = [i64::from(at[0]), i64::from(at[1])].map(q3::to_real);
        let leaf = GROUND.cell_of(LEAF_ZOOM, &point(x, y, 0.0)).tile().at;
        area::past(&source::ServiceArea, leaf)
    }

    /// Whether `space` may hold an edit. Three things must hold:
    ///
    /// - every shard under it is read, so a write stands over what its file
    ///   held (domain/layer/object/repository.rs `holds`);
    /// - none of its leaf columns is the wall (pxl-engine `area::walls_in`),
    ///   as `within` requires of a walker;
    /// - `space` lies over the bedrock and under the ceiling of ground the
    ///   world knows exactly, with no tile still missing (pxl-spec limit.rs).
    ///
    /// The bedrock and the ceiling rise with the ground's face, so every
    /// column's limits hold where the lowest ceiling and the highest bedrock
    /// do. Those come from the ground's bound over the nodes that hold
    /// `space` at its leaf cells: the fewest nodes at the zoom of its widest
    /// side, at most two a side (`TileCache::relief`). An aligned cube is one
    /// node, its own columns. A box that is not aligned is bounded by the
    /// nodes around it, which can only refuse more.
    pub fn editable(&self, space: &Aabb) -> bool {
        repository::holds(space) && self.editable_in(&source::ServiceArea, space)
    }

    /// `editable` against the service area `area`, without the check that
    /// the shards are read.
    pub(super) fn editable_in(&self, area: &impl Area, space: &Aabb) -> bool {
        let (cell_zoom, unit) = (GROUND.leaf_cell_zoom(), units(GROUND.leaf_cell_zoom()));
        let first = |k: usize| (space.min[k] / unit).floor() as i32;
        let cnt = |k: usize| ((space.max[k] / unit).ceil() as i32 - first(k)).max(1);
        let last = |k: usize| first(k) + cnt(k) - 1;
        if walls_in(area, [[first(0), first(1)], [last(0), last(1)]]) {
            return false;
        }
        let s = (cnt(0).max(cnt(1)) as u64)
            .next_power_of_two()
            .trailing_zeros();
        let zoom = cell_zoom - s;
        let nodes = |k: usize| first(k) >> s..=(first(k) + cnt(k) - 1) >> s;
        nodes(1).all(|y| {
            nodes(0).all(|x| {
                let node = (zoom, [wrap_idx(zoom, x), y]);
                let (line, mut place) = (self.terrain.cursor(node, cell_zoom), Lookup::NONE);
                // `lo` and `hi` are the ground's lowest and highest faces in
                // the node, in leaf cells.
                self.terrain
                    .relief(&line, &mut place, node, [cell_zoom, cell_zoom])
                    .exact()
                    .is_some_and(|[[lo, hi], _]| {
                        f64::from(bedrock(hi)) * unit <= space.min[2]
                            && space.max[2] <= f64::from(ceiling(lo)) * unit
                    })
            })
        })
    }
}

pub(super) fn within_in(area: &impl Area, at: &Q3) -> bool {
    let [x, y] = [i64::from(at[0]), i64::from(at[1])].map(q3::to_real);
    let (cell_zoom, b) = GROUND.cell_of(LEAF_ZOOM, &point(x, y, 0.0)).node();
    !is_wall(Shard::UNKNOWN.at(area, (cell_zoom, b)), cell_zoom, b)
}
