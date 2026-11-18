// pxl-core/domain/layer/terrain/tile.rs

//! A terrain tile as the core holds it: what its packet decodes to (pxl-spec
//! `decode_into`), and its lines divided among its bricks, once as it lands,
//! for each brick's fill to read its own of (clipmap.rs `take_lines`). All of
//! it goes when the tile does.

use pxl_spec::bake::publication::BaseKind;
use pxl_spec::bake::tile::terrain::{self, BrickPairs};
use pxl_spec::bake::tile::{Occupancy, Tile};
use std::collections::TryReserveError;

#[derive(Default)]
pub(crate) struct TerrainTile {
    pub(super) tile: terrain::TerrainTile,
    pub(super) pairs: BrickPairs,
}

impl TerrainTile {
    /// `tile` with its lines divided among its bricks, which a heap may
    /// refuse the room.
    pub(crate) fn of(tile: terrain::TerrainTile) -> Result<TerrainTile, TryReserveError> {
        let pairs = BrickPairs::of(&tile.separation)?;
        Ok(TerrainTile { tile, pairs })
    }
}

impl Tile for TerrainTile {
    const KIND: BaseKind = BaseKind::Terrain;

    fn occupancy(&self) -> Occupancy {
        self.tile.occupancy
    }
}
