// pxl-geo-bake/terrain.rs

pub mod absent;
pub mod bake;
pub mod leaf;
pub mod pipeline;
pub mod pool;
pub mod repository;
pub(crate) mod source;

use crate::pyramid::Base;
use crate::pyramid::Staged;
use pxl_engine::clipmap::BRICK;
use pxl_engine::way::tree::{BRICK_CHILD_GROUPS, ROOT_NODES};
use pxl_engine::way::SPANS;
use pxl_spec::bake::fold::fold_terrain;
use pxl_spec::bake::publication::BaseLayer;
use pxl_spec::bake::separation::{MAX_LINES, MAX_POINTS};
use pxl_spec::bake::tile::terrain::{self, Fit, TerrainTile, Unfit};
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

    fn encode(&self, at: Quadrant) -> Result<Vec<u8>, String> {
        self.encode_with_fit(at).map(|(pkt, _)| pkt)
    }

    /// Refused where a record's room does not hold its lines (pxl-spec
    /// `Separation::fits`), or a window's brick would hold more of its
    /// ways' entries than a client gives one (pxl-spec `terrain::ways_fit`),
    /// or would not fill or build as a client's does (`terrain::fill_fit`);
    /// what that build takes is its fit (pool.rs `line`).
    fn encode_with_fit(&self, at: Quadrant) -> Result<(Vec<u8>, Option<String>), String> {
        self.separation
            .fits()
            .map_err(|[lines, points]| format!("its ways hold {lines} lines and {points} points, past MAX_LINES ({MAX_LINES}) or MAX_POINTS ({MAX_POINTS})"))?;
        terrain::ways_fit(&self.separation, BRICK)
            .map_err(|([qx, qy], n)| format!("its brick ({qx}, {qy}) holds {n} of the ways' entries, past WAYS_BRICK_SEGMENTS ({})", terrain::WAYS_BRICK_SEGMENTS))?;
        let fit = terrain::fill_fit(at, self, |_, _| {}).map_err(|(at, why)| unfit(at, why))?;
        let pkt = terrain::encode(self).map_err(|e| format!("{e:?}"))?;
        Ok((pkt, (fit != Fit::default()).then(|| pool::line(&fit))))
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

    /// Refused where the set's ways' trees would not fit the client's pool
    /// wherever the eye is (`pool::check`).
    fn check_pool(
        set: &Staged<'_>,
        workers: usize,
    ) -> Result<Option<Vec<(Quadrant, String)>>, String> {
        pool::check(set, workers).map(Some)
    }
}

/// Why brick `at` of a tile does not fit (`terrain::fill_fit`), said.
fn unfit([qx, qy]: [u32; 2], why: Unfit) -> String {
    match why {
        Unfit::Spans(spans) => format!("its brick ({qx}, {qy}) makes a column of {spans} of the ways' spans, past SPANS ({SPANS})"),
        Unfit::Nodes(nodes) => format!("its brick ({qx}, {qy}) has its ways meet {nodes} of its nodes along z, past ROOT_NODES ({ROOT_NODES})"),
        Unfit::ChildGroups => format!("its brick ({qx}, {qy}) takes more than BRICK_CHILD_GROUPS ({BRICK_CHILD_GROUPS}) of its ways' tree's child_groups"),
    }
}
