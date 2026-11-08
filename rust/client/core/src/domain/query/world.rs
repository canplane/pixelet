// pxl-core/domain/query/world.rs

//! The world's answers: the ground's and the placed objects' over it, in one
//! place. Objects are all resident, so what the world knows is what the
//! ground knows.

use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::Node;

use pxl_spec::space::layer::octant::Octant;

use super::changes::Footprint;
use crate::domain::layer::object::store;
use crate::domain::layer::object::store::ObjectQuery as _;
use crate::domain::layer::object::store::{Store, Walk};
use crate::domain::layer::terrain::TerrainQuery as _;
use crate::domain::layer::tile_cache::{Lines, TileCache};
use crate::domain::layer::Answer;
use crate::domain::layer::Base as _;
use pxl_engine::clipmap::FIRST_CELL;
use pxl_engine::LEAF_CELL;
use pxl_spec::asset::decor::DECORS;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::coord::aabb::Aabb;

/// The world's query: the resident ground and the placed objects, read
/// as one.
#[derive(Clone, Copy)]
pub struct World<'a> {
    pub(super) terrain: &'a TileCache<TerrainTile>,
    pub(super) object: &'a Store,
    /// The decor struck, as the boxes it claims: what is placed, and of what
    /// the ground generates the kinds whose bits are set, a bit a kind in
    /// `DECORS`' order; none where a ray passes through decor.
    pub(super) decor: Option<u32>,
    /// Whether it is unplaced, nothing placed asked of it: the ground's alone
    /// (pxl-engine `world::Inheritance::UNPLACED`).
    pub(super) unplaced: bool,
    /// What of what is placed a preview hides, and what it cuts to air.
    pub(super) hide: Option<Octant>,
    pub(super) carve: Option<Octant>,
    /// Where what its queries read is reported, if anywhere.
    pub(super) reading: Option<&'a Footprint>,
}

impl World<'static> {
    /// The world as stored: what every mechanic asks, the pick, a walk, the
    /// camera's clearance, an edit's reads and decor's column surface.
    pub fn now() -> World<'static> {
        let object: &'static Store = store::object();
        World::new(TerrainTile::tile_cache(), object)
    }
}

impl<'a> World<'a> {
    /// As it is, and decor is struck too, as the frame draws it about the
    /// eye: what is placed, and what the ground generates of each kind the
    /// frame generates at the leaves' clip level, by its zooms as the frame
    /// is handed them (domain/view/lens.rs `decor_zooms`). The pointer's pick,
    /// which alone asks what a decor thing is.
    pub fn striking_decor(self, zooms: &[u32; DECORS.len()]) -> World<'a> {
        let leaves = |z: u32| z >> (LEAF_CELL - FIRST_CELL) & 1;
        let kinds = zooms
            .iter()
            .enumerate()
            .fold(0, |bits, (k, &z)| bits | leaves(z) << k);
        World {
            decor: Some(kinds),
            ..self
        }
    }

    pub fn new(terrain: &'a TileCache<TerrainTile>, object: &'a Store) -> World<'a> {
        World {
            terrain,
            object,
            decor: None,
            unplaced: false,
            hide: None,
            carve: None,
            reading: None,
        }
    }

    /// As it is unplaced: nothing placed and no preview's cuts asked of it,
    /// the ground's alone, which decor generates on (pxl-engine
    /// `generation::DecorQuery::column_surface`).
    pub(super) fn ground_only(self) -> World<'a> {
        World {
            unplaced: true,
            hide: None,
            carve: None,
            ..self
        }
    }

    /// As a preview shows it: `hide` left out, `carve` cut to air, at the
    /// entry's zoom or finer, where it is drawn as held; coarser, the stored
    /// fold stands.
    pub fn previewing(self, hide: Option<Octant>, carve: Option<Octant>) -> World<'a> {
        World {
            hide,
            carve,
            ..self
        }
    }

    /// As it is, the columns its queries read reported into `footprint`.
    pub fn reading(self, footprint: &'a Footprint) -> World<'a> {
        World {
            reading: Some(footprint),
            ..self
        }
    }

    /// The cells of `zoom` the box from `a` to `b` lies in, read
    /// (`reading`).
    pub(super) fn read(&self, a: [f64; 2], b: [f64; 2], zoom: Zoom) {
        if let Some(c) = self.reading {
            c.read(a, b, zoom);
        }
    }

    /// The columns `columns`, read (`reading`): what a cached answer read.
    pub(super) fn read_columns(&self, columns: [[f64; 2]; 2]) {
        if let Some(c) = self.reading {
            c.join(columns);
        }
    }
}

/// What a walk over columns carries from one to the next (`World::bounds`):
/// the ground's lines and the objects' tree, each stepped down from where
/// the last left off. Good while neither moves.
#[derive(Default)]
pub struct ColumnWalk {
    ground: Lines,
    object: Walk,
}

impl World<'_> {
    /// Where the ground and what is placed stand in the node, as its cells
    /// at `cell` are read; each stepped from where `walked` last went.
    pub fn bounds(
        &self,
        walked: &mut ColumnWalk,
        node: Node<2>,
        cell: Zoom,
    ) -> Answer<Option<Aabb>> {
        let object = self.object.column(&mut walked.object, node);
        self.terrain
            .bound_walked(&mut walked.ground, node, cell)
            .map(|ground| ground.into_iter().chain(object).reduce(Aabb::join))
    }
}
