// pxl-core/link/export/source.rs

//! The source interface's method: the tile source the client declares.

use crate::domain::layer::Base as _;
use crate::domain::layer::terrain::tile::IndexedTerrainTile;
use crate::sys::io::source;
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_kit::status::errno::Errno;

/// Declares the source. Its top is the coarsest zoom the ground is drawn
/// with, so the ground answers otherwise everywhere from then on, which is
/// reported as a change of the world's one root.
pub(super) fn declare(min_zoom: u32, formats: Vec<u32>, shards: Vec<String>) -> Result<(), Errno> {
    let was = source::min_zoom();
    source::declare(min_zoom, &formats, shards.iter().map(String::as_str))?;
    if source::min_zoom() != was {
        IndexedTerrainTile::tile_cache().changed_at(Quadrant::new(0, [0, 0]));
    }
    Ok(())
}
