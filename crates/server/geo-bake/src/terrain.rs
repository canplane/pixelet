// pxl-geo-bake/terrain.rs

pub mod absent;
pub mod bake;
pub mod leaf;
pub mod pipeline;
pub mod repository;
pub mod source;

use pxl_spec::bake::format::{decode_terrain, encode_terrain, Terrain};
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::GROUND;

/// The ground cell's side: what the raster, the deck lines and a height count in.
pub const CELL_U: f64 = units(GROUND.cell());

impl crate::pyramid::Layer for Terrain {
    fn encode(&self) -> Result<Vec<u8>, String> {
        encode_terrain(self).map_err(|e| format!("{e:?}"))
    }

    fn decode(raw: &[u8]) -> Result<Self, String> {
        decode_terrain(raw).map_err(|e| format!("{e:?}"))
    }
}
