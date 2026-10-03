// pxl-geo-bake/terrain/source.rs

//! One page's source, read for the bake; the offline tool and the dev server
//! both bake through it.

use postgres::Client;
use pxl_spec::bake::constant::ELE_PADDED_SPAN;
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::ivec::IVec;
use xpute_core::status::error::MarshalError;

use crate::terrain::artifact::Surface;
use crate::terrain::bake::bake_surface;
use crate::terrain::repository::TerrainRepository;

const ELE_PADDED_SPAN2: usize = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;

/// Survivable per page, visible only in bulk: a page off the elevation raster
/// bakes flat (`flat_ele`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceStats {
    pub flat_ele: u64,
}

pub fn fetch_surface(repo: &TerrainRepository, client: &mut Client, page_idx: IVec, stats: &mut SourceStats) -> Result<Surface, BakeError> {
    let source = repo.fetch_page_source(client, epsg3857::get_page_bbox(page_idx))?;
    let ele = source.ele.clone().unwrap_or_else(|| {
        stats.flat_ele += 1;
        vec![0i16; ELE_PADDED_SPAN2]
    });
    Ok(bake_surface(page_idx, ele, &source))
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
