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
use pxl_spec::bake::tile::terrain::{self, Fit, TerrainTile, Unfit};
use pxl_spec::bake::tile::Occupancy;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{neighborhood, Tile};
use pxl_spec_core::way::tree::{ROOT_NODES, SQUARE_FAMILIES};
use pxl_spec_core::way::SPANS;
use pxl_spec_core::window::SQUARE;

/// The ground cell's side: what the raster, the separation's lines and a height count in.
pub const CELL_U: f64 = units(GROUND.cell());

impl crate::pyramid::Base for TerrainTile {
    const LAYER: pxl_spec::bake::published::BaseLayer =
        pxl_spec::bake::published::BaseLayer::terrain;

    const LATTICE: pxl_spec::space::layer::lattice::Lattice = GROUND;

    fn encode(&self, at: Tile) -> Result<Vec<u8>, String> {
        self.encode_stored(at).map(|(packet, _)| packet)
    }

    /// Refused where a record's room does not hold its lines (pxl-spec
    /// `Separation::fits`), or a window's square would hold more of its
    /// ways' entries than a client gives one (pxl-spec `terrain::ways_fit`),
    /// or would not fill or build as a client's does (`terrain::fill_fit`);
    /// what that build takes is kept beside the tile (pool.rs `line`).
    fn encode_stored(&self, at: Tile) -> Result<(Vec<u8>, Option<String>), String> {
        self.separation
            .fits()
            .map_err(|[lines, points]| format!("its ways hold {lines} lines and {points} points, past MAX_LINES ({MAX_LINES}) or MAX_POINTS ({MAX_POINTS})"))?;
        terrain::ways_fit(&self.separation, SQUARE)
            .map_err(|([qx, qy], n)| format!("its square ({qx}, {qy}) holds {n} of the ways' entries, past WAYS_SQUARE_SEGMENTS ({})", terrain::WAYS_SQUARE_SEGMENTS))?;
        let fit = terrain::fill_fit(at, self, |_, _| {}).map_err(|(at, why)| unfit(at, why))?;
        let packet = terrain::encode(self).map_err(|e| format!("{e:?}"))?;
        Ok((packet, (fit != Fit::default()).then(|| pool::line(&fit))))
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
    fn fits_set(
        set: &crate::pyramid::Staged<'_>,
        workers: usize,
    ) -> Result<Option<Vec<(Tile, String)>>, String> {
        pool::fits(set, workers).map(Some)
    }
}

/// Why square `at` of a tile does not fit (`terrain::fill_fit`), said.
pub fn unfit([qx, qy]: [u32; 2], why: Unfit) -> String {
    match why {
        Unfit::Spans(spans) => format!("its square ({qx}, {qy}) makes a column of {spans} of the ways' spans, past SPANS ({SPANS})"),
        Unfit::Nodes(nodes) => format!("its square ({qx}, {qy}) has its ways meet {nodes} of its nodes along z, past ROOT_NODES ({ROOT_NODES})"),
        Unfit::Families => format!("its square ({qx}, {qy}) takes more than SQUARE_FAMILIES ({SQUARE_FAMILIES}) of its ways' tree's families"),
    }
}
