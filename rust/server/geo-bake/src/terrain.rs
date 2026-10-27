// pxl-geo-bake/terrain.rs

pub mod absent;
pub mod bake;
pub mod leaf;
pub mod pipeline;
pub mod pool;
pub mod repository;
pub mod source;

use pxl_spec::bake::fold::fold_terrain;
use pxl_spec::bake::separation::{MAX_LINES, MAX_POINTS};
use pxl_spec::bake::tile::terrain::{self, TerrainTile};
use pxl_spec::bake::tile::Occupancy;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{neighborhood, Tile};
use pxl_spec_core::ways::tree::ROOT_NODES;
use pxl_spec_core::ways::SPANS;
use pxl_spec_core::window::SQUARE;

/// The ground cell's side: what the raster, the separation's lines and a height count in.
pub const CELL_U: f64 = units(GROUND.cell());

impl crate::pyramid::Base for TerrainTile {
    const LAYER: pxl_spec::bake::published::BaseLayer =
        pxl_spec::bake::published::BaseLayer::terrain;

    const LATTICE: pxl_spec::space::layer::lattice::Lattice = GROUND;

    /// Refused where a record's room does not hold its lines (pxl-spec
    /// `Separation::fits`), or a window's square would hold more of its
    /// ways' entries than a client gives one (pxl-spec `terrain::ways_fit`),
    /// or a column more of its ways' spans, or its ways more of its nodes
    /// along z than its tree's root holds (`terrain::fill_fit`).
    fn encode(&self, at: Tile) -> Result<Vec<u8>, String> {
        self.separation
            .fits()
            .map_err(|[lines, points]| format!("its ways hold {lines} lines and {points} points, past MAX_LINES ({MAX_LINES}) or MAX_POINTS ({MAX_POINTS})"))?;
        terrain::ways_fit(&self.separation, SQUARE)
            .map_err(|([qx, qy], n)| format!("its square ({qx}, {qy}) holds {n} of the ways' entries, past WAYS_SQUARE_SEGMENTS ({})", terrain::WAYS_SQUARE_SEGMENTS))?;
        terrain::fill_fit(at, self).map_err(|([qx, qy], [spans, nodes])| {
            format!("its square ({qx}, {qy}) makes a column of {spans} of the ways' spans and its ways meet {nodes} of its nodes along z, past SPANS ({SPANS}) or ROOT_NODES ({ROOT_NODES})")
        })?;
        terrain::encode(self).map_err(|e| format!("{e:?}"))
    }

    fn decode(raw: &[u8]) -> Result<Self, String> {
        terrain::decode(raw).map_err(|e| format!("{e:?}"))
    }

    /// The ring around the four children too, for the elevation padding.
    fn sources(p: Tile) -> Vec<Tile> {
        neighborhood(p).to_vec()
    }

    fn fold_tile<'a>(p: Tile, held: impl Fn(Tile) -> Option<&'a Self>) -> Option<Self> {
        fold_terrain(neighborhood(p).map(held), p.zoom).map(|t| terrain::drawn(p, t))
    }

    fn occupancy(&self) -> Occupancy {
        self.occupancy
    }

    /// Refused where the set's ways' trees would not fit the client's pool
    /// wherever the eye is (`pool::fits`).
    fn fits_set(set: &crate::pyramid::Staged<'_>, workers: usize) -> Result<(), String> {
        pool::fits(set, workers)
    }
}
