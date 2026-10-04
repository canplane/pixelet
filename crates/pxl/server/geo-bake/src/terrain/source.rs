// pxl-geo-bake/terrain/source.rs

//! One page's source, read for the bake; the offline tool and the dev server
//! both bake through it.

use postgres::Client;
use pxl_spec::bake::format::ELE_LANE;
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::ivec::IVec;

use crate::deck::Carried;
use crate::terrain::artifact::Surface;
use crate::terrain::bake::bake_surface;
use crate::terrain::repository::TerrainRepository;

/// The page's surface, and whether it lies off the elevation raster and so
/// bakes flat: survivable a page, visible only in bulk.
pub fn fetch_surface(repo: &TerrainRepository, client: &mut Client, page_idx: IVec, drawn: &[Drawn]) -> Result<(Surface, bool), String> {
    let mut source = repo.fetch_page_source(client, epsg3857::get_page_bbox(page_idx)).map_err(|e| e.to_string())?;
    let flat = source.ele.is_none();
    let ele = source.ele.take().unwrap_or_else(|| vec![0; ELE_LANE]);
    Ok((bake_surface(page_idx, ele, &source, drawn), flat))
}

pub fn fetch_surface(repo: &TerrainRepository, client: &mut Client, page_idx: IVec, carried: &[Carried], stats: &mut SourceStats) -> Result<Surface, BakeError> {
    let source = repo.fetch_page_source(client, epsg3857::get_page_bbox(page_idx))?;
    let ele = source.ele.clone().unwrap_or_else(|| {
        stats.flat_ele += 1;
        vec![0i16; ELE_PADDED_SPAN2]
    });
    Ok(bake_surface(page_idx, ele, &source, carried))
}

/// The page's source, assembled straight out of the database.
pub fn fetch_terrain_source(repo: &TerrainRepository, client: &mut Client, page_idx: IVec, stats: &mut SourceStats) -> Result<Vec<u8>, BakeError> {
    let page_bbox = epsg3857::get_page_bbox(page_idx);
    let source = repo.fetch_page_source(client, page_bbox)?;
    Ok(pack_source(&source, stats)?)
}

/// Raw Overture and elevation source, baked into client-ready ground channels.
pub fn fetch_tile(repo: &TerrainRepository, client: &mut Client, page_idx: IVec, stats: &mut SourceStats) -> Result<Vec<u8>, BakeError> {
    let raw = fetch_terrain_source(repo, client, page_idx, stats)?;
    Ok(encode_terrain(&bake_terrain(page_idx, &raw)?)?)
}

#[derive(Debug)]
pub enum BakeError {
    Db(postgres::Error),
    Marshal(MarshalError),
}

impl From<postgres::Error> for BakeError {
    fn from(e: postgres::Error) -> BakeError {
        BakeError::Db(e)
    }
}

impl From<MarshalError> for BakeError {
    fn from(e: MarshalError) -> BakeError {
        BakeError::Marshal(e)
    }
}

impl std::fmt::Display for BakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BakeError::Db(e) => write!(f, "db: {e}"),
            BakeError::Marshal(e) => write!(f, "packet: {e:?}"),
        }
    }
}

impl std::error::Error for BakeError {}
