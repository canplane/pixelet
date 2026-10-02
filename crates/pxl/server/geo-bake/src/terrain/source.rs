// pxl-geo-bake/terrain/source.rs

//! One page's raw source packed for the bake; the offline tool and the dev
//! server share this so their tiles are byte-identical.

use postgres::Client;
use pxl_spec::bake::constant::ELE_PADDED_SPAN;
use pxl_spec::bake::deck::Decks;
use pxl_spec::bake::format::{BaseRow, BuildingRow, CoastRow, SegmentRow};
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::ivec::IVec;
use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{encoder, BranchView, BranchViewImpl, NodeView, TreeEncoderOptions, TreeView};

use crate::terrain::artifact::Surface;
use crate::terrain::bake::bake_surface;
use crate::terrain::pipeline::deck::bake_decks;
use crate::terrain::repository::{PageSource, TerrainRepository};

const ELE_PADDED_SPAN2: usize = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;

/// Survivable per page, visible only in bulk: a page off the elevation raster
/// bakes flat (`flat_ele`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceStats {
    pub flat_ele: u64,
}

fn rows_branch<'a, F: FnMut(&mut BranchViewImpl<'a>)>(mut each: F) -> TreeView<'a> {
    let mut view = TreeView::new();
    view.branch(Some(&mut each));
    view
}

fn building_tuple<'a>(b: &mut BranchViewImpl<'a>, row: &'a BuildingRow) {
    b.branch(Some(&mut |t| {
        t.str(Some(&row.gers_id))
            .str(row.subtype.as_deref())
            .str(row.class.as_deref())
            .f64(row.height)
            .f64(row.num_floors)
            .u8_array(Some(&row.geom_wkb));
    }));
}

fn segment_tuple<'a>(b: &mut BranchViewImpl<'a>, row: &'a SegmentRow) {
    b.branch(Some(&mut |t| {
        t.str(Some(&row.gers_id))
            .str(Some(&row.subtype))
            .str(row.class.as_deref())
            .str(row.flags.as_deref())
            .str(row.widths.as_deref())
            .str(row.access.as_deref())
            .str(row.subclass.as_deref())
            .str(row.subclasses.as_deref())
            .u8_array(Some(&row.geom_wkb));
    }));
}

fn base_tuple<'a>(b: &mut BranchViewImpl<'a>, row: &'a BaseRow) {
    b.branch(Some(&mut |t| {
        t.str(Some(&row.gers_id))
            .str(Some(&row.source))
            .str(row.subtype.as_deref())
            .str(row.class.as_deref())
            .u8_array(Some(&row.geom_wkb));
    }));
}

fn coast_tuple<'a>(b: &mut BranchViewImpl<'a>, row: &'a CoastRow) {
    b.branch(Some(&mut |t| {
        t.str(Some(&row.id)).u8_array(Some(&row.geom_wkb)).bool(Some(row.covers_page));
    }));
}

pub fn pack_source(source: &PageSource, stats: &mut SourceStats) -> Result<Vec<u8>, MarshalError> {
    let ele = match &source.ele {
        Some(v) => v.clone(),
        None => {
            stats.flat_ele += 1;
            vec![0i16; ELE_PADDED_SPAN2]
        }
    };

    let buildings = rows_branch(|b| {
        for row in &source.building_rows {
            building_tuple(b, row);
        }
    });
    let segments = rows_branch(|b| {
        for row in &source.segment_rows {
            segment_tuple(b, row);
        }
    });
    let base = rows_branch(|b| {
        for row in &source.base_rows {
            base_tuple(b, row);
        }
    });
    let coasts = rows_branch(|b| {
        for row in &source.coast_rows {
            coast_tuple(b, row);
        }
    });

    let mut root = TreeView::new();
    root.branch(Some(&mut |r| {
        r.i16_array(Some(&ele));
        r.subtree(&buildings);
        r.subtree(&segments);
        r.subtree(&base);
        r.subtree(&coasts);
    }));

    encoder().encode(&root, TreeEncoderOptions::default())
}

pub fn fetch_terrain_source(repo: &TerrainRepository, client: &mut Client, page_idx: IVec, stats: &mut SourceStats) -> Result<Vec<u8>, BakeError> {
    let page_bbox = epsg3857::get_page_bbox(page_idx);
    let source = repo.fetch_page_source(client, page_bbox)?;
    Ok(pack_source(&source, stats)?)
}

pub fn fetch_surface(repo: &TerrainRepository, client: &mut Client, page_idx: IVec, stats: &mut SourceStats) -> Result<Surface, BakeError> {
    let raw = fetch_terrain_source(repo, client, page_idx, stats)?;
    Ok(bake_surface(page_idx, &raw)?)
}

pub fn fetch_lines(repo: &TerrainRepository, client: &mut Client, page_idx: IVec) -> Result<Decks, BakeError> {
    let page_bbox = epsg3857::get_page_bbox(page_idx);
    let rows = repo.fetch_deck_rows(client, page_bbox)?;
    bake_decks(&rows, page_bbox[0]).ok_or(BakeError::Marshal(MarshalError::new(Errno::ENOSPC)))
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
