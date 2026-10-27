// pxl-kernel/domain/query/surface.rs

//! The ground's surface: a cell's top and the block its face is, read off
//! the windows by the one reading of a column the GPU reads too
//! (pxl-spec-core `window::ground`), the area's edge laid on it. The
//! ground's alone, so a part of the support (`support.rs`), where a walker
//! starts looking for what it stands on, and read inside the query only:
//! what a cell holds, a cave, a way, a bridge or what is placed included,
//! is the world's answer (`World::asset_of`), never the ground's.

use pxl_spec::bake::tile::terrain::GroundId;
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec_core::area::Shard;
use pxl_spec_core::window::Found;

use super::world::World;
use crate::domain::layer::terrain::window::Windowed;
use crate::domain::layer::{Answer, Query};

/// The ground's top in a cell, in units, and the block its face is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    pub top: f64,
    pub asset: GroundId,
}

impl World<'_> {
    /// The surface of the cell `c` of `cell`: its ground's top, in units,
    /// and its face; past the service area the Earth's, at sea level; none
    /// where it has no ground.
    pub(super) fn surface_of(&self, cell: Zoom, c: [i32; 2]) -> Answer<Option<Surface>> {
        let shard = Shard::UNKNOWN.at(&Windowed, (cell, c));
        if shard.past() {
            return Answer::Exact(Some(Surface {
                top: f64::from(pxl_spec_core::SEA_LEVEL),
                asset: pxl_spec::asset::cube::sentinel::unknown,
            }));
        }
        let side = units(cell);
        let middle = c.map(|v| (f64::from(v) + 0.5) * side);
        self.read(middle, middle, cell);
        let line = self.terrain.cursor((cell, c), cell);
        self.terrain
            .column(&line, Found::NONE, (cell, c))
            .map(|col| {
                col.grounded().then(|| Surface {
                    top: f64::from(col.top) * units(cell),
                    asset: col.look as GroundId,
                })
            })
    }
}
