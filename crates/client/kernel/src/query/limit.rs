// pxl-kernel/query/limit.rs

//! Where the world ends for what moves in it: the service area, which a
//! walker does not leave, and where a block may be placed in or taken from,
//! over ground the world knows between its bedrock and its ceiling. Every
//! walk and every edit asks these, and nothing else decides them.

use pxl_spec::block::cube::sentinel;
use pxl_spec::block::limit::ceiling;
use pxl_spec::block::stand::Block;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::world::point;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use super::block::Surface;
use super::World;
use crate::layer::Answer;
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;

impl World<'_> {
    /// Whether the leaf column under `at` is the service area's: its surface
    /// has a look, as every column of the area's does and none past it, the
    /// bedrock wall's included.
    pub fn within(&self, at: &Pvf3) -> Answer<bool> {
        self.runs_at(at, LEAF_ZOOM, |_, surface| in_area(surface))
    }

    /// Whether `space` may hold an edit: every leaf column under it is ground
    /// the world knows exactly — the service area's, not the unknown past it,
    /// nor a tile not yet held — and it lies over that column's bedrock and
    /// under its ceiling (`pxl-spec block/limit.rs`).
    pub fn editable(&self, space: &Aabb) -> bool {
        let unit = units(GROUND.cell());
        let cells = |k: usize| (space.min[k] / unit).floor() as i64..(space.max[k] / unit).ceil() as i64;
        cells(1).all(|y| {
            cells(0).all(|x| {
                let center = point((x as f64 + 0.5) * unit, (y as f64 + 0.5) * unit, space.min[2]);
                let open = self.runs_at(&center, LEAF_ZOOM, |runs, surface| {
                    let bedrock = runs.iter().filter(|r| r.block == Block::Ground(sentinel::unknown)).all(|r| r.hi <= space.min[2]);
                    in_area(surface) && surface.is_some_and(|s| space.max[2] <= ceiling(s.top)) && bedrock
                });
                open.exact() == Some(true)
            })
        })
    }
}

/// A surface of a look: the bedrock walling the area has one too.
fn in_area(surface: Option<Surface>) -> bool {
    surface.is_some_and(|s| s.block != sentinel::unknown)
}
