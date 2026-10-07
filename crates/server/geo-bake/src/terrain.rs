// pxl-geo-bake/terrain.rs

pub mod absent;
pub mod bake;
pub mod coast;
pub mod leaf;
pub mod pipeline;
pub mod repository;
pub mod source;

use pxl_spec::bake::fold::fold_terrain;
use pxl_spec::bake::format::{decode_terrain, encode_terrain, Occupancy, Terrain};
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{neighborhood, Tile};

/// The ground cell's side: what the raster, the deck lines and a height count in.
pub const CELL_U: f64 = units(GROUND.cell());

impl crate::pyramid::Layer for Terrain {
    fn encode(&self) -> Result<Vec<u8>, String> {
        encode_terrain(self).map_err(|e| format!("{e:?}"))
    }

    fn decode(raw: &[u8]) -> Result<Self, String> {
        decode_terrain(raw).map_err(|e| format!("{e:?}"))
    }

    /// The ring around the four children too, for the elevation padding.
    fn sources(p: Tile) -> Vec<Tile> {
        neighborhood(p).to_vec()
    }

    fn fold_tile<'a>(p: Tile, held: impl Fn(Tile) -> Option<&'a Self>) -> Option<Self> {
        fold_terrain(neighborhood(p).map(held), p.zoom)
    }

    fn occupancy(&self) -> Occupancy {
        self.index.occupancy
    }
}
