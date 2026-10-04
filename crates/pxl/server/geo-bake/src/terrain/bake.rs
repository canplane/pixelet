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
}
