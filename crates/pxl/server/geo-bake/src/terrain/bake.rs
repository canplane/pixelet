// pxl-geo-bake/terrain/bake.rs

//! One page's raw source packet, baked into the client-ready ground channels.
//!
//! The packet is what terrain/source.rs assembles from the database:
//!
//!   [ele, buildings, segments, base, coasts]
//!
//! with each row a tuple in the order pxl-spec's `*_ROW_KEYS` name. Those
//! arrays are the encoder's; a reader knows the order by position, so the rows
//! are read here by index against the same lists.

use pxl_spec::bake::format::{encode_terrain, BaseRow, BuildingRow, CoastRow, Ground, SegmentRow};
use pxl_spec::bake::model::feature::Feature;
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::ivec::IVec;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{BranchCursor, SequenceType, TreeReader};

use crate::terrain::pipeline::coast::adapter::from_coast_row;
use crate::terrain::pipeline::coast::policy::CoastPolicy;
use crate::terrain::pipeline::ground::bake_ground;
use crate::terrain::pipeline::normalizer::normalize;
use crate::terrain::pipeline::overture::adapter::{from_base_row, from_building_row, from_segment_row};
use crate::terrain::pipeline::overture::policy::OverturePolicy;

fn opt_str(row: &BranchCursor, at: u32) -> Result<Option<String>, MarshalError> {
    match row.at(at)?.opt() {
        Some(c) => Ok(Some(c.get_str()?)),
        None => Ok(None),
    }
}

/// A number column, which the packet may carry as text.
///
/// `overture_buildings.height` is `double precision`, and the Deno driver
/// decodes those as strings rather than numbers — the same thing the forward
/// search had to undo on its own coordinates. So the row says `f64` and the
/// packet holds a `str` for every building that has a height, while
/// `num_floors` is an `integer` and arrives as a number. Both forms are
/// read.
///
/// Nothing downstream reads the result — `FeatureComputed::height` reaches no
/// rasterizer — so this is about decoding the packet that exists rather than
/// about the tile.
fn opt_num(row: &BranchCursor, at: u32) -> Result<Option<f64>, MarshalError> {
    let Some(c) = row.at(at)?.opt() else { return Ok(None) };
    if c.type_() == SequenceType::STR as u8 {
        // An unparseable string is NaN, which the caller clamps away.
        return Ok(Some(c.get_str()?.trim().parse::<f64>().unwrap_or(f64::NAN)));
    }
    Ok(Some(c.get_number()?))
}

fn building_row(row: &BranchCursor<'_>) -> Result<BuildingRow, MarshalError> {
    Ok(BuildingRow {
        gers_id: row.at(0)?.get_str()?,
        subtype: opt_str(row, 1)?,
        class: opt_str(row, 2)?,
        height: opt_num(row, 3)?,
        num_floors: opt_num(row, 4)?,
        geom_wkb: row.at(5)?.get_array::<u8>()?.into(),
    })
}

fn segment_row(row: &BranchCursor<'_>) -> Result<SegmentRow, MarshalError> {
    Ok(SegmentRow {
        gers_id: row.at(0)?.get_str()?,
        subtype: row.at(1)?.get_str()?,
        class: opt_str(row, 2)?,
        road: opt_str(row, 3)?,
        geom_wkb: row.at(4)?.get_array::<u8>()?.into(),
    })
}

fn base_row(row: &BranchCursor<'_>) -> Result<BaseRow, MarshalError> {
    Ok(BaseRow {
        gers_id: row.at(0)?.get_str()?,
        source: row.at(1)?.get_str()?,
        subtype: opt_str(row, 2)?,
        class: opt_str(row, 3)?,
        geom_wkb: row.at(4)?.get_array::<u8>()?.into(),
    })
}

fn coast_row(row: &BranchCursor<'_>) -> Result<CoastRow, MarshalError> {
    Ok(CoastRow {
        id: row.at(0)?.get_str()?,
        geom_wkb: row.at(1)?.get_array::<u8>()?.into(),
        covers_page: row.at(2)?.get_bool()?,
    })
}

pub fn bake_terrain(page_idx: IVec, pkt: &[u8]) -> Result<Vec<u8>, MarshalError> {
    let page_bbox = epsg3857::get_page_bbox(page_idx);
    let page_min = page_bbox[0];
    let page_z_scale = epsg3857::Z_SCALE(page_bbox[0][1]);

    let root = TreeReader::new(pkt)?.read_branch()?;

    let ele: &[i16] = root.at(0)?.get_array::<i16>()?;
    let buildings = root.at(1)?.as_branch()?;
    let segments = root.at(2)?.as_branch()?;
    let base = root.at(3)?.as_branch()?;
    let coasts = root.at(4)?.as_branch()?;

    let mut feats: Vec<Feature> = Vec::new();

    for i in 0..buildings.len {
        for raw in from_building_row(&building_row(&buildings.at_branch(i)?)?) {
            feats.extend(normalize(&raw, page_min, page_z_scale, &OverturePolicy));
        }
    }
    for i in 0..segments.len {
        for raw in from_segment_row(&segment_row(&segments.at_branch(i)?)?) {
            feats.extend(normalize(&raw, page_min, page_z_scale, &OverturePolicy));
        }
    }
    for i in 0..base.len {
        for raw in from_base_row(&base_row(&base.at_branch(i)?)?) {
            feats.extend(normalize(&raw, page_min, page_z_scale, &OverturePolicy));
        }
    }
    for i in 0..coasts.len {
        for raw in from_coast_row(&coast_row(&coasts.at_branch(i)?)?) {
            feats.extend(normalize(&raw, page_min, page_z_scale, &CoastPolicy));
        }
    }

    // Whether *any* coastline row touched this page's envelope at all, even if
    // from_coast_row dropped it (covers_page => full land). bake_ground needs
    // this to tell "no land data at all (open ocean)" apart from "land data
    // present but not worth rasterizing (full land)".
    let has_coast_row = coasts.len > 0;

    let ground: Ground = bake_ground(&feats, has_coast_row);

    encode_terrain(ele, &ground)
}
