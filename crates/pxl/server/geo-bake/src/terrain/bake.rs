// pxl-geo-bake/terrain/bake.rs

//! One page's source rows baked into its surface.

use pxl_spec::bake::format::Ground;
use pxl_spec::bake::model::feature::Feature;
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::ivec::IVec;

use crate::deck::Drawn;
use crate::terrain::artifact::Surface;
use crate::terrain::pipeline::coast::adapter::from_coast_row;
use crate::terrain::pipeline::coast::policy::CoastPolicy;
use crate::terrain::pipeline::corridor::corridor_fills;
use crate::terrain::pipeline::ground::bake_ground;
use crate::terrain::pipeline::normalizer::normalize;
use crate::terrain::pipeline::overture::adapter::{from_base_row, from_building_row, from_segment_row};
use crate::terrain::pipeline::overture::policy::OverturePolicy;
use crate::terrain::repository::PageSource;

/// `drawn`: the ranges of the page's ways its lines draw.
pub fn bake_surface(page_idx: IVec, ele: Vec<i16>, source: &PageSource, drawn: &[Drawn]) -> Surface {
    let page_min = epsg3857::get_page_bbox(page_idx)[0];
    let mut feats: Vec<Feature> = Vec::new();
    for row in &source.building_rows {
        feats.extend(from_building_row(row).iter().filter_map(|raw| normalize(raw, page_min, &OverturePolicy)));
    }
    for row in &source.segment_rows {
        feats.extend(from_segment_row(row, drawn).iter().filter_map(|raw| normalize(raw, page_min, &OverturePolicy)));
    }
    for row in &source.base_rows {
        feats.extend(from_base_row(row).iter().filter_map(|raw| normalize(raw, page_min, &OverturePolicy)));
    }
    for row in &source.coast_rows {
        feats.extend(from_coast_row(row).iter().filter_map(|raw| normalize(raw, page_min, &CoastPolicy)));
    }

    // Tells open ocean (no coast row) from full land (a row dropped as covering the page).
    let has_coast_row = !source.coast_rows.is_empty();

    let fills = corridor_fills(&feats);
    feats.extend(fills);
    let ground: Ground = bake_ground(&feats, has_coast_row);
    Surface { ele, ground }
}
