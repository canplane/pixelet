// pxl-kernel/query/surface.rs

//! The ground's surface: a cell's top and the block its face is, read off
//! the windows by the one reading of a column the GPU reads too
//! (pxl-spec-core `window::ground`), the area's edge laid on it. What a
//! policy over the ground — where a walker stands over it — reads, knowing
//! no tile.

use pxl_spec::bake::tile::terrain::GroundId;
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec_core::area::Shard;
use pxl_spec_core::window::Found;

use super::world::World;
use crate::layer::terrain::window::Windowed;
use crate::layer::{Answer, Query};

/// The ground's top in a cell, in units, and the block its face is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    pub top: f64,
    pub block: GroundId,
}

impl World<'_> {
    /// The surface of the cell `c` of `cell`: its ground's top, in units,
    /// and its face; past the service area the Earth's, at sea level; none
    /// where it has no ground.
    pub fn surface_of(&self, cell: Zoom, c: [i32; 2]) -> Answer<Option<Surface>> {
        let shard = Shard::UNKNOWN.at(&Windowed, (cell, c));
        if shard.past() {
            return Answer::Exact(Some(Surface {
                top: f64::from(pxl_spec_core::SEA_LEVEL),
                block: pxl_spec::block::cube::sentinel::unknown,
            }));
        }
        let line = self.terrain.cursor((cell, c), cell);
        self.terrain.column(&line, Found::NONE, (cell, c)).map(|col| {
            col.grounded().then(|| Surface {
                top: f64::from(col.top) * units(cell),
                block: col.look as GroundId,
            })
        })
    }
}
