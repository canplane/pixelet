// pxl-geo-bake/terrain/source.rs

//! One tile's source, read for the bake; the offline tool and the dev server
//! both bake through it.

use postgres::Client;
use pxl_spec::space::coord::adapter::epsg3857;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use crate::separation::Drawn;
use crate::terrain::bake::bake_surface;
use crate::terrain::bake::Surface;
use crate::terrain::repository::TerrainRepository;

/// The tile's surface.
pub fn fetch_surface(repo: &TerrainRepository, client: &mut Client, tile_idx: [i64; 2], drawn: &[Drawn]) -> Result<Surface, String> {
    let source = repo.fetch_tile_source(client, epsg3857::get_tile_bbox(LEAF_ZOOM, tile_idx)).map_err(|e| e.to_string())?;
    Ok(bake_surface(tile_idx, source, drawn))
}
