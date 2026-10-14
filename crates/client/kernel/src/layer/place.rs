// pxl-kernel/layer/place.rs

//! Places as a layer of the pyramid: what OpenStreetMap Carto would draw of
//! a tile at its zoom, read where the camera grid's cell is a cell of the
//! tile, an icon wide, and drawn as Carto draws it (`chunk/place.rs`).

use pxl_spec::bake::tile::place::{self, PlaceTile, PLACE_LATTICE};
use pxl_spec::space::layer::lattice::Lattice;

use crate::io::demand::{Phases, Stream};
use crate::io::failure::Failure;
use crate::layer::residency::Residency;
use crate::layer::Base;
use pxl_spec::bake::tile::Occupancy;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::tile::Tile;
use xpute_guest::global::Global;

static PLACE: Global<Residency<PlaceTile>> = Global::new();

impl Base for PlaceTile {
    const STREAM: Stream = Stream::place;
    const LATTICE: Lattice = PLACE_LATTICE;

    fn occupancy(&self) -> Occupancy {
        self.occupancy
    }
    fn residency() -> &'static mut Residency<PlaceTile> {
        PLACE.get(Residency::default)
    }

    /// Small, so held on the heap; the record is only a count slot.
    fn receive(_tile: Tile, _record: u32, pkt: &[u8], _phases: &mut Phases) -> Result<PlaceTile, Failure> {
        Ok(place::decode(pkt)?)
    }

    fn empty() -> Option<PlaceTile> {
        Some(PlaceTile::default())
    }
}

/// Carto's zoom a grid cell at `cell` is drawn at: the tile zoom whose cell,
/// an icon wide, it is, past the leaf too.
pub fn carto_zoom(cell: Zoom) -> i32 {
    PLACE_LATTICE.tile_zoom_of_grid(cell)
}
