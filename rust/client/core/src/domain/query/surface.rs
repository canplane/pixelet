// pxl-core/domain/query/surface.rs

//! The ground's surface: a cell's top and the asset of its face, read off
//! the clip levels by the same reading of a column the GPU uses (pxl-engine
//! `clipmap::ground`). Where the column is the wall at the service area's
//! edge (pxl-spec `area::is_wall`), the face is the wall's.
//!
//! The surface is the ground's alone. The support (`support.rs`) uses it as
//! the place a body starts looking for what it stands on, and nothing
//! outside the query reads it. What a cell holds, including a cave, a way, a
//! bridge or what is placed, is the world's answer (`World::asset_of`),
//! never the ground's.

use crate::domain::layer::Base as _;
use crate::domain::layer::terrain::tile::IndexedTerrainTile;
use pxl_engine::clipmap::Lookup;
use pxl_spec::ground::GroundId;
use pxl_spec::space::coord::world::SEA_LEVEL;
use pxl_spec::space::coord::zoom::{Zoom, units};
use pxl_spec::space::layer::area::Shard;

use super::change;
use super::world::World;
use crate::domain::layer::Answer;
use crate::domain::layer::terrain::clipmap::clip_read;
use pxl_spec::asset::cube::Sentinel;

/// The ground's top in a cell, in units, and the asset of its face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Surface {
    pub top: f64,
    pub asset_id: GroundId,
}

impl World {
    /// The surface of the cell `c` at `cell_zoom`. Past the service area it
    /// is the Earth's, at sea level. None where the cell has no ground.
    pub(super) fn surface_of(&self, cell_zoom: Zoom, c: [i32; 2]) -> Answer<Option<Surface>> {
        let shard = Shard::UNKNOWN.at(&clip_read(), (cell_zoom, c));
        if shard.past() {
            return Answer::Exact(Some(Surface {
                top: f64::from(SEA_LEVEL),
                asset_id: Sentinel::Unknown as u8,
            }));
        }
        let side = units(cell_zoom);
        let middle = c.map(|v| (f64::from(v) + 0.5) * side);
        change::report_read(middle, middle, cell_zoom);
        let line = IndexedTerrainTile::tile_cache().cursor((cell_zoom, c), cell_zoom);
        IndexedTerrainTile::tile_cache()
            .column(&line, Lookup::NONE, (cell_zoom, c))
            .map(|col| {
                col.is_grounded().then(|| Surface {
                    top: f64::from(col.top) * units(cell_zoom),
                    asset_id: col.asset_id as GroundId,
                })
            })
    }
}
