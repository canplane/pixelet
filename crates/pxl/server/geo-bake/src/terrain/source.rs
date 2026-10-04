// pxl-geo-bake/terrain/source.rs

//! One page's source, read for the bake; the offline tool and the dev server
//! both bake through it.

use postgres::Client;
use pxl_spec::bake::format::ELE_LANE;
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::ivec::IVec;

use crate::deck::Drawn;
use crate::terrain::bake::Surface;
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

fn deck_tuple<'a>(b: &mut BranchViewImpl<'a>, row: &'a DeckRow) {
    b.branch(Some(&mut |t| {
        t.str(Some(&row.gers_id))
            .str(Some(&row.subtype))
            .str(row.class.as_deref())
            .f64(Some(row.width_m))
            .u8_array(Some(&row.piers))
            .u8_array(Some(&row.geom_wkb));
    }));
}

fn coast_tuple<'a>(b: &mut BranchViewImpl<'a>, row: &'a CoastRow) {
    b.branch(Some(&mut |t| {
        t.str(Some(&row.id)).u8_array(Some(&row.geom_wkb)).bool(Some(row.covers_page));
    }));
}

/// Packs what the database gave into `[ele, buildings, segments, base, coasts, decks]`,
/// each row a tuple in the order of its row type's fields (pxl-spec
/// bake/format.rs).
pub fn pack_source(source: &PageSource, stats: &mut SourceStats) -> Result<Vec<u8>, MarshalError> {
    let ele = match &source.ele {
        Some(v) if v.len() == ELE_PADDED_SPAN2 => v.clone(),
        _ => {
            stats.invalid_ele_shape += 1;
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

    let decks = rows_branch(|b| {
        for row in &source.deck_rows {
            deck_tuple(b, row);
        }
    });

    let mut root = TreeView::new();
    root.branch(Some(&mut |r| {
        r.i16_array(Some(&ele));
        r.subtree(&buildings);
        r.subtree(&segments);
        r.subtree(&base);
        r.subtree(&coasts);
        r.subtree(&decks);
    }));

    encoder().encode(&root, TreeEncoderOptions::default())
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
