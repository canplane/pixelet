// pxl-core/domain/layer/terrain/tile.rs

//! A terrain tile as the core holds it: what its packet decodes to (pxl-spec
//! `decode_into`), and its lines divided among its bricks, once as it lands,
//! for each brick's fill to read its own of (clipmap.rs `take_lines`). All of
//! it goes when the tile does.

use pxl_spec::bake::publication::BaseKind;
use pxl_spec::bake::tile::terrain::{self, BrickEntries};
use pxl_spec::bake::tile::{Occupancy, Tile};
use std::collections::TryReserveError;

#[derive(Default)]
pub(crate) struct IndexedTerrainTile {
    pub(super) tile: terrain::TerrainTile,
    pub(super) entries: BrickEntries,
}

impl IndexedTerrainTile {
    /// `tile` with its lines divided among its bricks, which a heap may
    /// refuse the room.
    pub(crate) fn of(tile: terrain::TerrainTile) -> Result<IndexedTerrainTile, TryReserveError> {
        let entries = BrickEntries::of(&tile.separation)?;
        Ok(IndexedTerrainTile { tile, entries })
    }
}

impl Tile for IndexedTerrainTile {
    const KIND: BaseKind = BaseKind::Terrain;

    fn occupancy(&self) -> Occupancy {
        self.tile.occupancy
    }
}
