// pxl-geo-bake/terrain.rs

pub mod absent;
pub mod bake;
pub mod leaf;
pub mod pipeline;
pub mod repository;
pub mod source;

use pxl_spec::bake::fold::fold_terrain;
use pxl_spec::bake::tile::terrain::{self, TerrainTile};
use pxl_spec::bake::tile::Occupancy;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::Tile;

/// The ground cell's side: what the raster, the separation's lines and a height count in.
pub const CELL_U: f64 = units(GROUND.cell());

impl crate::pyramid::Base for TerrainTile {
    const LAYER: pxl_spec::bake::published::BaseLayer = pxl_spec::bake::published::BaseLayer::terrain;

    fn encode(&self) -> Result<Vec<u8>, String> {
        terrain::encode(self).map_err(|e| format!("{e:?}"))
    }

    fn decode(raw: &[u8]) -> Result<Self, String> {
        terrain::decode(raw).map_err(|e| format!("{e:?}"))
    }

    fn fold_tile<'a>(p: Tile, held: impl Fn(Tile) -> Option<&'a Self>) -> Option<Self> {
        fold_terrain(p.children().map(held), p.zoom)
    }

    fn occupancy(&self) -> Occupancy {
        self.occupancy
    }
}
