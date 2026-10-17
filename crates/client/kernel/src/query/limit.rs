// pxl-kernel/query/limit.rs

//! Where the world ends for what moves in it: the service area, the shards
//! its source serves (io/source.rs), which a walker does not leave, and where
//! a block may be placed in or taken from, over ground the world knows
//! between its bedrock and its ceiling. Every walk and every edit asks these,
//! and nothing else decides them.

use pxl_portable::area::Shard;
use pxl_portable::wrap_index;
use pxl_spec::block::limit::{at_edge, bedrock, ceiling};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::world::point;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::{Cells, GROUND};
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use super::World;
use crate::io::source;
use crate::layer::Query;
use pxl_portable::window::Found;
use pxl_spec::space::coord::q3::{self, Q3};

impl World<'_> {
    /// Whether the leaf cell under `at` is the service area's and off its
    /// edge (`pxl-spec block/limit.rs` `at_edge`), where the wall stands:
    /// known before any of its ground arrives, off its shard's nine as the
    /// fill lays the wall (pxl-portable `area::gone_ring`), the area's index
    /// asked once (`Shard::at`).
    pub fn within(&self, at: &Q3) -> bool {
        let [x, y] = [i64::from(at[0]), i64::from(at[1])].map(q3::to_real);
        within_tile(GROUND.cell_of(LEAF_ZOOM, &point(x, y, 0.0)).tile().at.map(|v| v as i32))
    }

    /// Whether `space` may hold an edit: every leaf tile under it within
    /// the service area and off its edge (`within`), and there `space` over
    /// the bedrock and under the ceiling of ground the world knows exactly,
    /// no tile not yet held (`pxl-spec block/limit.rs`). The bedrock and
    /// the ceiling rise with the face, so every column's hold where the
    /// lowest ceiling and the highest bedrock do: the ground's bound over
    /// the nodes holding `space` at its leaf cells, the fewest at the zoom
    /// of its widest side, two a side at most (`Residency::relief`). An
    /// aligned cube is one node, its own columns; a box that is not is
    /// bounded by the nodes about it, which only refuses more.
    pub fn editable(&self, space: &Aabb) -> bool {
        let (cell, unit) = (GROUND.cell(), units(GROUND.cell()));
        let first = |k: usize| (space.min[k] / unit).floor() as i64;
        let count = |k: usize| ((space.max[k] / unit).ceil() as i64 - first(k)).max(1);
        // The leaf tiles under it, the area a fact of each.
        let t = GROUND.tile_cell_shamt();
        let tiles = |k: usize| first(k) >> t..=(first(k) + count(k) - 1) >> t;
        if !tiles(1).all(|y| tiles(0).all(|x| within_tile([wrap_index(LEAF_ZOOM, x as i32), y as i32]))) {
            return false;
        }
        let s = (count(0).max(count(1)) as u64).next_power_of_two().trailing_zeros();
        let zoom = cell - s;
        let nodes = |k: usize| first(k) >> s..=(first(k) + count(k) - 1) >> s;
        nodes(1).all(|y| {
            nodes(0).all(|x| {
                let node = (zoom, [i64::from(wrap_index(zoom, x as i32)), y]);
                let (line, mut place) = (self.terrain.cursor(node, cell), Found::NONE);
                // The ground's faces, in leaf cells.
                self.terrain
                    .relief(&line, &mut place, node)
                    .exact()
                    .is_some_and(|[[lo, hi], _]| f64::from(bedrock(hi)) * unit <= space.min[2] && space.max[2] <= ceiling(f64::from(lo) * unit))
            })
        })
    }
}

/// Whether the leaf tile `leaf` and the eight about it are the service
/// area's (`within`).
fn within_tile(leaf: [i32; 2]) -> bool {
    let node = (LEAF_ZOOM, leaf);
    let shard = Shard::UNKNOWN.at(&source::Declared, node);
    !at_edge(|dx, dy| shard.past_leaf(node, [wrap_index(LEAF_ZOOM, leaf[0] + dx as i32), leaf[1] + dy as i32]))
}
