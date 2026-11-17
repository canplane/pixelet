// pxl-core/domain/query/world.rs

//! The world's answers: the ground and the placed objects over it, read as
//! one. The objects are all resident, so what the world knows is what the
//! ground knows.

use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::Node;

use crate::domain::layer::Answer;
use crate::domain::layer::Base as _;
use crate::domain::layer::object::store;
use crate::domain::layer::object::store::{Store, Walk};
use crate::domain::layer::tile_cache::Path;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::coord::aabb::Aabb;

#[derive(Clone, Copy)]
pub(crate) struct World {
    pub(super) object: &'static Store,
}

impl World {
    /// The world as stored. Every mechanic asks it: the pick, a walk, the
    /// camera's clearance, an edit's reads and decor's column surface.
    pub(crate) fn now() -> World {
        World {
            object: store::object(),
        }
    }

    /// The base: the world with nothing placed in it.
    pub(crate) fn base() -> World {
        World {
            object: store::empty(),
        }
    }
}

/// What a walk over columns carries from one column to the next
/// (`World::bound`): the ground's lines and the objects' tree, each stepped
/// down from where the last column left off. It stays valid only while
/// neither the ground nor the objects move.
#[derive(Default)]
pub(crate) struct ColumnWalk {
    ground: Path,
    object: Walk,
}

impl World {
    /// The box the ground and what is placed fill in the node, as its cells
    /// at `cell_zoom` are read. The walk steps on from where `walk` last
    /// went.
    pub(crate) fn bound(
        &self,
        walk: &mut ColumnWalk,
        node: Node<2>,
        cell_zoom: Zoom,
    ) -> Answer<Option<Aabb>> {
        let object = self.object.column(&mut walk.object, node);
        TerrainTile::tile_cache()
            .bound_walked(&mut walk.ground, node, cell_zoom)
            .map(|ground| ground.into_iter().chain(object).reduce(Aabb::join))
    }
}
