// pxl-geo-bake/terrain/bake.rs

//! One tile's source rows baked into its surface, which with its lines is the tile.

use pxl_spec::bake::deck::Decks;
use pxl_spec::bake::format::{Ground, Terrain};
use pxl_spec::space::coord::adapter::epsg3857;
use pxl_spec::space::coord::pvf3::ivec::IVec;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use crate::deck::Drawn;
use crate::terrain::pipeline::feature::{local, Feature};
use crate::terrain::pipeline::ground::{bake_ground, Coast};
use crate::terrain::pipeline::overture::policy::{bases, buildings, ways};
use crate::terrain::repository::TileSource;
use crate::wkb::wkb_to_geometries;

pub struct Surface {
    pub ele: Vec<i16>,
    pub ground: Ground,
}

pub fn assemble(surface: Surface, lines: Decks) -> Terrain {
    Terrain::leaf(surface.ele, surface.ground, lines)
}

/// `drawn`: the ranges of the tile's ways its lines draw.
pub fn bake_surface(tile_idx: IVec, ele: Vec<i16>, source: &TileSource, drawn: &[Drawn]) -> Surface {
    let tile_min = epsg3857::get_tile_bbox(LEAF_ZOOM, tile_idx)[0];
    let mut features: Vec<Feature> = Vec::new();
    features.extend(source.building_rows.iter().flat_map(|row| buildings(row, tile_min)));
    features.extend(source.segment_rows.iter().flat_map(|row| ways(row, drawn, tile_min)));
    features.extend(source.base_rows.iter().flat_map(|row| bases(row, tile_min)));

    // no coast row is open sea; one whose land covers the tile is dropped, all land
    let shore: Vec<_> = source
        .coast_rows
        .iter()
        .filter(|row| !row.covers_tile)
        .flat_map(|row| wkb_to_geometries(&row.geom_wkb))
        .filter(|g| !g.points.is_empty())
        .map(|g| {
            (
                g.points.iter().map(|&p| local(p, tile_min)).collect(),
                g.holes.iter().map(|ring| ring.iter().map(|&p| local(p, tile_min)).collect()).collect(),
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
