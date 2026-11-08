// pxl-core/domain/query/world.rs

//! The world's answers: the ground and the placed objects over it, read as
//! one. The objects are all resident, so what the world knows is what the
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
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::coord::aabb::Aabb;

#[derive(Clone, Copy)]
pub struct World<'a> {
    pub(super) terrain: &'a TileCache<TerrainTile>,
    pub(super) object: &'a Store,
    /// Whether a ray strikes decor, as the boxes it claims. Otherwise rays
    /// pass through decor.
    pub(super) decor: bool,
    /// Whether the world is read unplaced: the ground alone, with nothing
    /// placed asked of it (pxl-engine `world::Inheritance::UNPLACED`).
    pub(super) unplaced: bool,
    /// What a preview hides of what is placed (`hide`), and what it cuts to
    /// air (`carve`).
    pub(super) hide: Option<Octant>,
    pub(super) carve: Option<Octant>,
    /// Where the queries report what they read, if anywhere.
    pub(super) reading: Option<&'a Footprint>,
}

impl World<'static> {
    /// The world as stored. Every mechanic asks it: the pick, a walk, the
    /// camera's clearance, an edit's reads and decor's column surface.
    pub fn now() -> World<'static> {
        let object: &'static Store = store::object();
        World::new(TerrainTile::tile_cache(), object)
    }

    /// This world with decor struck too, where the frame holds it. Only the
    /// pointer's pick asks for this, since it alone asks what a decor thing
    /// is.
    pub fn striking_decor(self) -> World<'static> {
        World {
            decor: true,
            ..self
        }
    }
}

impl<'a> World<'a> {
    pub fn new(terrain: &'a TileCache<TerrainTile>, object: &'a Store) -> World<'a> {
        World {
            terrain,
            object,
            decor: false,
            unplaced: false,
            hide: None,
            carve: None,
            reading: None,
        }
    }

    /// This world unplaced: the ground alone, with nothing placed and no
    /// preview's cuts. Decor is generated on it (pxl-engine
    /// `generation::DecorQuery::column_surface`).
    pub(super) fn ground_only(self) -> World<'a> {
        World {
            unplaced: true,
            hide: None,
            carve: None,
            ..self
        }
    }

    /// This world as a preview shows it: `hide` left out and `carve` cut to
    /// air. The cuts apply at the entry's zoom or finer, where the world is
    /// drawn as held; at coarser zooms the stored fold stands.
    pub fn previewing(self, hide: Option<Octant>, carve: Option<Octant>) -> World<'a> {
        World {
            hide,
            carve,
            ..self
        }
    }

    /// This world with the columns its queries read reported into
    /// `footprint`.
    pub fn reading(self, footprint: &'a Footprint) -> World<'a> {
        World {
            reading: Some(footprint),
            ..self
        }
    }

    /// Reports the cells of `zoom` that the box from `a` to `b` lies in
    /// (`reading`).
    pub(super) fn read(&self, a: [f64; 2], b: [f64; 2], zoom: Zoom) {
        if let Some(c) = self.reading {
            c.read(a, b, zoom);
        }
    }

    /// Reports `columns` as read (`reading`). A kept answer reports what it
    /// read this way.
    pub(super) fn read_columns(&self, columns: [[f64; 2]; 2]) {
        if let Some(c) = self.reading {
            c.join(columns);
        }
    }
}

/// What a walk over columns carries from one column to the next
/// (`World::bounds`): the ground's lines and the objects' tree, each stepped
/// down from where the last column left off. It stays valid only while
/// neither the ground nor the objects move.
#[derive(Default)]
pub struct ColumnWalk {
    ground: Lines,
    object: Walk,
}

impl World<'_> {
    /// The box the ground and what is placed fill in the node, as its cells
    /// at `cell_zoom` are read. The walk steps on from where `walked` last
    /// went.
    pub fn bounds(
        &self,
        walked: &mut ColumnWalk,
        node: Node<2>,
        cell_zoom: Zoom,
    ) -> Answer<Option<Aabb>> {
        let object = self.object.column(&mut walked.object, node);
        self.terrain
            .bound_walked(&mut walked.ground, node, cell_zoom)
            .map(|ground| ground.into_iter().chain(object).reduce(Aabb::join))
    }
}
