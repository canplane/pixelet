// pxl-geo-bake/terrain/source.rs

//! One page's source, read for the bake; the offline tool and the dev server
//! both bake through it.

use postgres::Client;
use pxl_spec::bake::format::ELE_LANE;
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::ivec::IVec;

use crate::deck::Drawn;
use crate::terrain::bake::bake_surface;
use crate::terrain::bake::Surface;
use crate::terrain::repository::TerrainRepository;

/// The page's surface, and whether it lies off the elevation raster and so
/// bakes flat: survivable a page, visible only in bulk.
pub fn fetch_surface(repo: &TerrainRepository, client: &mut Client, page_idx: IVec, drawn: &[Drawn]) -> Result<(Surface, bool), String> {
    let mut source = repo.fetch_page_source(client, epsg3857::get_page_bbox(page_idx)).map_err(|e| e.to_string())?;
    let flat = source.ele.is_none();
    let ele = source.ele.take().unwrap_or_else(|| vec![0; ELE_LANE]);
    Ok((bake_surface(page_idx, ele, &source, drawn), flat))
}
