// pxl-geo-bake/terrain/bake.rs

//! One tile's source rows baked into its surface, which with its lines is the tile.

use pxl_spec::bake::separation::Separation;
use pxl_spec::bake::tile::terrain::{self, Ground, TerrainTile};
use pxl_spec::bake::way::lay;
use pxl_spec::procedural::Frame;
use pxl_spec::space::coord::adapter::epsg3857::{self, G3857};
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

use crate::separation::Drawn;
use crate::terrain::pipeline::feature::{local, Feature};
use crate::terrain::pipeline::ground::{bake_ground, Coast};
use crate::terrain::pipeline::overture::policy::{bases, buildings, ways};
use crate::terrain::repository::{CoastRow, TileSource};
use crate::wkb::wkb_to_geometries;

pub struct Surface {
    pub ele: Vec<i16>,
    pub ground: Ground,
}

/// The approaches' looks laid into its cells (pxl-spec bake/way.rs `lay`),
/// then its lines those it draws (pxl-spec bake/tile/terrain.rs `drawn`).
pub fn assemble(tile: [i32; 2], surface: Surface, lines: Separation) -> TerrainTile {
    let mut looks = surface.ground.cells.into_owned();
    let tile = Tile::new(LEAF_ZOOM, tile);
    lay(&lines, Frame::of(tile), &mut looks);
    terrain::drawn(
        tile,
        TerrainTile::leaf(
            surface.ele,
            Ground {
                cells: looks.into(),
            },
            lines,
        ),
    )
}

/// `drawn`: the ranges of the tile's ways its lines draw.
pub fn bake_surface(tile_idx: [i32; 2], src: TileSource, drawn: &[Drawn]) -> Surface {
    let tile_min = epsg3857::get_tile_bbox(LEAF_ZOOM, tile_idx)[0];
    let mut feats: Vec<Feature> = Vec::new();
    feats.extend(
        src.building_rows
            .iter()
            .flat_map(|row| buildings(row, tile_min)),
    );
    feats.extend(src.parts.iter().flat_map(|row| ways(row, drawn, tile_min)));
    feats.extend(src.base_rows.iter().flat_map(|row| bases(row, tile_min)));

    let ground = bake_ground(&feats, &coast_of(&src.coast_rows, tile_min));
    Surface {
        ele: src.ele,
        ground,
    }
}

/// A tile's land against the sea: no coast row is open sea; one whose land
/// covers the tile is dropped, all land.
fn coast_of(rows: &[CoastRow], tile_min: G3857) -> Coast {
    let at = |p: G3857| local(p, tile_min);
    let shore: Vec<_> = rows
        .iter()
        .filter(|row| !row.covers_tile)
        .flat_map(|row| wkb_to_geometries(&row.geom_wkb))
        .filter(|g| !g.points.is_empty())
        .map(|g| {
            (
                g.points.iter().map(|&p| at(p)).collect(),
                g.holes
                    .iter()
                    .map(|ring| ring.iter().map(|&p| at(p)).collect())
                    .collect(),
            )
        })
        .collect();
    match (rows.is_empty(), shore.is_empty()) {
        (true, _) => Coast::Sea,
        (false, true) => Coast::Land,
        (false, false) => Coast::Shore(shore),
    }
}
