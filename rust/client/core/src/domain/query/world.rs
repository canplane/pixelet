// pxl-core/domain/query/world.rs

//! The world's answers: the ground and the placed objects over it, read as
//! one. The objects are all resident, so what the world knows is what the
//! ground knows.

use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::Node;

use crate::domain::layer::object::store;
use crate::domain::layer::object::store::{Store, Walk};
use crate::domain::layer::tile_cache::{Path, TileCache};
use crate::domain::layer::Answer;
use crate::domain::layer::Base as _;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::coord::aabb::Aabb;

#[derive(Clone, Copy)]
pub struct World<'a> {
    pub(super) terrain: &'a TileCache<TerrainTile>,
    pub(super) object: &'a Store,
}

impl World<'static> {
    /// The world as stored. Every mechanic asks it: the pick, a walk, the
    /// camera's clearance, an edit's reads and decor's column surface.
    pub fn now() -> World<'static> {
        let object: &'static Store = store::object();
        World::new(TerrainTile::tile_cache(), object)
    }
}

impl<'a> World<'a> {
    /// The base: this world with nothing placed in it.
    pub fn base(self) -> World<'a> {
        World {
            object: store::nothing_placed(),
            ..self
        }
    }

    pub fn new(terrain: &'a TileCache<TerrainTile>, object: &'a Store) -> World<'a> {
        World { terrain, object }
    }
}

/// What a walk over columns carries from one column to the next
/// (`World::bounds`): the ground's lines and the objects' tree, each stepped
/// down from where the last column left off. It stays valid only while
/// neither the ground nor the objects move.
#[derive(Default)]
pub struct ColumnWalk {
    ground: Path,
    object: Walk,
}

impl World<'_> {
    /// The box the ground and what is placed fill in the node, as its cells
    /// at `cell_zoom` are read. The walk steps on from where `walk` last
    /// went.
    pub fn bounds(
        &self,
        walk: &mut ColumnWalk,
        node: Node<2>,
        cell_zoom: Zoom,
    ) -> Answer<Option<Aabb>> {
        let object = self.object.column(&mut walk.object, node);
        self.terrain
            .bound_walked(&mut walk.ground, node, cell_zoom)
            .map(|ground| ground.into_iter().chain(object).reduce(Aabb::join))
    }
}
