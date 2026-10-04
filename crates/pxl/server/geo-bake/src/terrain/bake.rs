// pxl-geo-bake/terrain/bake.rs

//! One page's source rows baked into its surface, which with its lines is the page.

use pxl_spec::bake::deck::Decks;
use pxl_spec::bake::format::{Ground, Terrain};
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::ivec::IVec;

use crate::deck::Drawn;
use crate::terrain::pipeline::corridor::corridor_fills;
use crate::terrain::pipeline::feature::{local, Feature};
use crate::terrain::pipeline::ground::{bake_ground, Coast};
use crate::terrain::pipeline::overture::policy::{bases, buildings, ways};
use crate::terrain::repository::PageSource;
use crate::wkb::wkb_to_geometries;

pub struct Surface {
    pub ele: Vec<i16>,
    pub ground: Ground,
}

pub fn assemble(surface: Surface, lines: Decks) -> Terrain {
    Terrain::leaf(surface.ele, surface.ground, lines)
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
