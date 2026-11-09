// pxl-geo-bake/terrain.rs

pub mod absent;
pub mod bake;
pub mod leaf;
pub mod pipeline;
pub mod repository;
pub(crate) mod source;

use crate::pyramid::Base;
use pxl_spec::bake::fold::fold_terrain;
use pxl_spec::bake::tile::terrain::{self, TerrainTile};
use pxl_spec::bake::tile::Tile;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::quadrant::{neighborhood, Quadrant};

/// The ground cell's side: what the raster, the separation's lines and an elevation count in.
pub(crate) const CELL_U: f64 = units(GROUND.leaf_cell_zoom());

impl Base for TerrainTile {
    /// The packet, refused past the limit a client holds one to
    /// (pxl-spec `BaseLayer::row`).
    fn encode(&self, at: Quadrant) -> Result<Vec<u8>, String> {
        let pkt = terrain::encode(self).map_err(|e| format!("{}: {e:?}", at.key()))?;
        let cap = Self::LAYER.row().pkt_cap;
        if pkt.len() > cap {
            return Err(format!("tile {} size {} > {cap}", at.key(), pkt.len()));
        }
        Ok(pkt)
    }

    fn decode(raw: &[u8]) -> Result<Self, String> {
        terrain::decode(raw).map_err(|e| format!("{e:?}"))
    }

    /// The ring around the four children too, for the elevation padding.
    fn sources(p: Quadrant) -> Vec<Quadrant> {
        neighborhood(p).to_vec()
    }

    fn fold_tile<'a>(p: Quadrant, get: impl Fn(Quadrant) -> Option<&'a Self>) -> Option<Self> {
        fold_terrain(neighborhood(p).map(get), p.zoom).map(|t| terrain::drawn(p, t))
    }
}

#[cfg(test)]
#[path = "terrain.test.rs"]
mod test;
