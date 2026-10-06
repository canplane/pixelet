// pxl-geo-bake/terrain/source.rs

//! One tile's source, read for the bake; the offline tool and the dev server
//! both bake through it.

use postgres::Client;
use pxl_spec::bake::format::ELE_LANE;
use pxl_spec::space::coord::adapter::epsg3857;
use pxl_spec::space::coord::pvf3::ivec::IVec;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use crate::deck::Drawn;
use crate::terrain::bake::bake_surface;
use crate::terrain::bake::Surface;
use crate::terrain::repository::TerrainRepository;

/// The tile's surface, and whether it lies off the elevation raster and so
/// bakes flat: survivable a tile, visible only in bulk.
pub fn fetch_surface(repo: &TerrainRepository, client: &mut Client, tile_idx: IVec, drawn: &[Drawn]) -> Result<(Surface, bool), String> {
    let mut source = repo.fetch_tile_source(client, epsg3857::get_tile_bbox(LEAF_ZOOM, tile_idx)).map_err(|e| e.to_string())?;
    let flat = source.ele.is_none();
    let ele = source.ele.take().unwrap_or_else(|| vec![0; ELE_LANE]);
    Ok((bake_surface(tile_idx, ele, &source, drawn), flat))
}
