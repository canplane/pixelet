// pxl-core/domain/layer/place.rs

//! The place layer: for each tile, the places OpenStreetMap Carto would draw
//! at the tile's zoom. A tile is read where one camera grid cell is one cell
//! of the tile, which is an icon wide, and its places are drawn as Carto
//! draws them (domain/scene/marker/place.rs).

use pxl_spec::bake::tile::place::{self, PlaceTile, PLACE_LATTICE};
use pxl_spec::space::layer::lattice::Lattice;

use crate::domain::layer::tile_cache::TileCache;
use crate::domain::layer::Base;
use crate::sys::io::demand::{PhaseList, Stream};
use crate::sys::io::failure::Exit;
use pxl_spec::bake::tile::Occupancy;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_core::global::Global;

static PLACE: Global<TileCache<PlaceTile>> = Global::new();

impl Base for PlaceTile {
    const STREAM: Stream = Stream::place;
    const LATTICE: Lattice = PLACE_LATTICE;

    fn occupancy(&self) -> Occupancy {
        self.occupancy
    }
    fn tile_cache() -> &'static mut TileCache<PlaceTile> {
        PLACE.get(TileCache::default)
    }

    /// A place tile is small, so it is decoded onto the heap, and its slot
    /// only counts it against the cache's capacity.
    fn receive(
        _tile: Quadrant,
        _slot: u32,
        pkt: &[u8],
        _phases: &mut PhaseList,
    ) -> Result<PlaceTile, Exit> {
        Ok(place::decode(pkt)?)
    }

    fn empty() -> Option<PlaceTile> {
        Some(PlaceTile::default())
    }
}

/// The Carto zoom that a grid cell at `cell_zoom` is drawn at: the tile zoom
/// whose cell, an icon wide, is that grid cell. It holds past the leaf zoom
/// too.
pub fn carto_zoom(cell_zoom: Zoom) -> i32 {
    PLACE_LATTICE.tile_zoom_of_grid(cell_zoom)
}
