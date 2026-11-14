// pxl-geo-bake/terrain/source.rs

//! One tile's source, read for the bake; the offline tool and the dev server
//! both bake through it.

use postgres::Client;

use crate::separation::WayRange;
use crate::terrain::bake::bake_surface;
use crate::terrain::bake::Surface;
use crate::terrain::pipeline::overture::policy::{INFRASTRUCTURE, LAND};
use crate::terrain::repository::TerrainRepository;

pub(crate) fn fetch_surface(
    repo: &TerrainRepository,
    client: &mut Client,
    tile_idx: [i32; 2],
    way_ranges: &[WayRange],
) -> Result<Surface, String> {
    // The rows are read only in the classes the policy paints.
    let [infrastructure, land]: [Vec<&str>; 2] =
        [INFRASTRUCTURE, LAND].map(|table| table.iter().map(|(class, _)| *class).collect());
    let src = repo
        .fetch_tile_src(client, tile_idx, [&infrastructure, &land])
        .map_err(|e| e.to_string())?;
    Ok(bake_surface(tile_idx, src, way_ranges))
}
