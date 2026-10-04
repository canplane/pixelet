// pxl-geo-bake/terrain/bake.rs

//! One page's raw source packet `[ele, buildings, segments, base, coasts]`
//! baked into its surface. Rows are tuples read by position (pxl-spec bake/format.rs).

use pxl_spec::bake::format::{BaseRow, BuildingRow, CoastRow, Ground, SegmentRow, Terrain};
use pxl_spec::bake::model::feature::Feature;
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::ivec::IVec;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{BranchCursor, SequenceType, TreeReader};

use crate::terrain::pipeline::coast::adapter::from_coast_row;
use crate::terrain::pipeline::coast::policy::CoastPolicy;
use crate::terrain::pipeline::corridor::corridor_fills;
use crate::terrain::pipeline::deck::bake_decks;
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

/// `overture_buildings.height` arrives as a string (the Deno driver decodes
/// doubles as text) and `num_floors` as a number; both forms are read.
fn opt_num(row: &BranchCursor, at: u32) -> Result<Option<f64>, MarshalError> {
    let Some(c) = row.at(at)?.opt() else { return Ok(None) };
    if c.type_() == SequenceType::STR as u8 {
        // An unparseable string is NaN, which the caller clamps away.
        return Ok(Some(c.get_str()?.trim().parse::<f64>().unwrap_or(f64::NAN)));
    }
    Ok(Some(c.get_number()?))
}

/// `drawn`: the ranges of the page's ways its lines draw.
pub fn bake_surface(page_idx: IVec, ele: Vec<i16>, source: &PageSource, drawn: &[Drawn]) -> Surface {
    let page_min = epsg3857::get_page_bbox(page_idx)[0];
    let mut features: Vec<Feature> = Vec::new();
    features.extend(source.building_rows.iter().flat_map(|row| buildings(row, page_min)));
    features.extend(source.segment_rows.iter().flat_map(|row| ways(row, drawn, page_min)));
    features.extend(source.base_rows.iter().flat_map(|row| bases(row, page_min)));
    let fills = corridor_fills(&features);
    features.extend(fills);

    // no coast row is open sea; one whose land covers the page is dropped, all land
    let shore: Vec<_> = source
        .coast_rows
        .iter()
        .filter(|row| !row.covers_page)
        .flat_map(|row| wkb_to_geometries(&row.geom_wkb))
        .filter(|g| !g.points.is_empty())
        .map(|g| {
            (
                g.points.iter().map(|&p| local(p, page_min)).collect(),
                g.holes.iter().map(|ring| ring.iter().map(|&p| local(p, page_min)).collect()).collect(),
            )
        })
        .collect();
    let coast = match (source.coast_rows.is_empty(), shore.is_empty()) {
        (true, _) => Coast::Sea,
        (false, true) => Coast::Land,
        (false, false) => Coast::Shore(shore),
    };
    Surface {
        ele,
        ground: bake_ground(&features, &coast),
    }
    for i in 0..segments.len {
        for raw in from_segment_row(&segment_row(&segments.at_branch(i)?)?) {
            feats.extend(normalize(&raw, page_min, &OverturePolicy));
        }
    }
    for i in 0..base.len {
        for raw in from_base_row(&base_row(&base.at_branch(i)?)?) {
            feats.extend(normalize(&raw, page_min, &OverturePolicy));
        }
    }
    for i in 0..coasts.len {
        for raw in from_coast_row(&coast_row(&coasts.at_branch(i)?)?) {
            feats.extend(normalize(&raw, page_min, &CoastPolicy));
        }
    }

    // Whether *any* coastline row touched this page's envelope at all, even if
    // from_coast_row dropped it (covers_page => full land). bake_ground needs
    // this to tell "no land data at all (open ocean)" apart from "land data
    // present but not worth rasterizing (full land)".
    let has_coast_row = coasts.len > 0;

    let fills = corridor_fills(&feats);
    feats.extend(fills);
    let ground: Ground = bake_ground(&feats, has_coast_row);

    let rows = (0..deck_rows.len).map(|i| deck_row(&deck_rows.at_branch(i)?)).collect::<Result<Vec<_>, _>>()?;
    let decks = bake_decks(&rows, page_min).ok_or(MarshalError::new(Errno::ENOSPC))?;

    Ok(Terrain::leaf(ele.to_vec(), ground, decks))
}
