// pxl-geo-bake/terrain/source.rs

//! One tile's source, read for the bake; the offline tool and the dev server
//! both bake through it.

use postgres::Client;

use crate::separation::WayRange;
use crate::terrain::bake::bake_surface;
use crate::terrain::bake::Surface;
use crate::terrain::repository::TerrainRepository;

pub fn fetch_surface(
    repo: &TerrainRepository,
    client: &mut Client,
    tile_idx: [i32; 2],
    way_ranges: &[WayRange],
) -> Result<Surface, String> {
    let src = repo
        .fetch_tile_src(client, tile_idx)
        .map_err(|e| e.to_string())?;
    Ok(bake_surface(tile_idx, src, way_ranges))
}
