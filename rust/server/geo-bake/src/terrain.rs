// pxl-geo-bake/terrain.rs

pub mod absent;
pub mod bake;
pub mod leaf;
pub mod pipeline;
pub mod repository;
pub(crate) mod source;

use crate::pyramid::Base;
use pxl_spec::bake::fold::fold_terrain;
use pxl_spec::bake::publication::BaseLayer;
use pxl_spec::bake::tile::max_pkt_bytes;
use pxl_spec::bake::tile::terrain::{self, TerrainTile};
use pxl_spec::bake::tile::Occupancy;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::Lattice;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::quadrant::{neighborhood, Quadrant};

/// The ground cell's side: what the raster, the separation's lines and a height count in.
pub(crate) const CELL_U: f64 = units(GROUND.leaf_cell_zoom());

impl Base for TerrainTile {
    const LAYER: BaseLayer = BaseLayer::terrain;

    const LATTICE: Lattice = GROUND;

    /// The packet, refused past the limit a client holds one to
    /// (pxl-spec `max_pkt_bytes`).
    fn encode(&self, at: Quadrant) -> Result<Vec<u8>, String> {
        let pkt = terrain::encode(self).map_err(|e| format!("{}: {e:?}", at.key()))?;
        let cap = max_pkt_bytes(Self::LAYER);
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

    fn occupancy(&self) -> Occupancy {
        self.occupancy
    }
}

#[cfg(test)]
#[path = "terrain.test.rs"]
mod test;
