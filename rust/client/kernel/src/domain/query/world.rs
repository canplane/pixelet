// pxl-kernel/domain/query/world.rs

//! The world's answers: the ground's and the placed objects' over it, in one
//! place. Objects are all resident, so what the world knows is what the
//! ground knows.

use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::Node;

use pxl_spec::bake::constant::EntryAt;

use super::changed::Columns;
use crate::domain::layer::object::store::{Store, Walk};
use crate::domain::layer::tile_cache::{Lines, TileCache};
use crate::domain::layer::Answer;
use crate::domain::layer::Base as _;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::coord::aabb::Aabb;

/// The world's query: the resident ground and the placed objects, read
/// as one.
#[derive(Clone, Copy)]
pub struct World<'a> {
    pub(super) terrain: &'a TileCache<TerrainTile>,
    pub(super) objects: &'a Store,
    /// Whether decor is struck, as the boxes it claims; else a ray passes
    /// through decor.
    pub(super) decor: bool,
    /// Whether it is unplaced, nothing placed asked of it: the ground's alone
    /// (pxl-spec-core `world::Handed::unplaced`).
    pub(super) unplaced: bool,
    /// What of what is placed a preview hides, and what it cuts to air.
    pub(super) hide: Option<EntryAt>,
    pub(super) carve: Option<EntryAt>,
    /// Where what its queries read is reported, if anywhere.
    pub(super) reading: Option<&'a Columns>,
}

impl World<'static> {
    /// The world as stored: what every mechanic asks, the pick, a walk, the
    /// camera's clearance, an edit's reads and decor's footing.
    pub fn now() -> World<'static> {
        let objects: &'static Store = crate::domain::layer::object::store::objects();
        World::new(TerrainTile::tile_cache(), objects)
    }

    /// As `now`, and decor is struck too, where the frame holds it: the
    /// pointer's pick, which alone asks what a decor thing is.
    pub fn striking_decor(self) -> World<'static> {
        World {
            decor: true,
            ..self
        }
    }
}

impl<'a> World<'a> {
    pub fn new(terrain: &'a TileCache<TerrainTile>, objects: &'a Store) -> World<'a> {
        World {
            terrain,
            objects,
            decor: false,
            unplaced: false,
            hide: None,
            carve: None,
            reading: None,
        }
    }

    /// As it is unplaced: nothing placed and no preview's cuts asked of it,
    /// the ground's alone, which decor grows on (pxl-spec-core
    /// `scatter::Grounds::footing`).
    pub(super) fn unplaced(self) -> World<'a> {
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
    pub fn previewing(self, hide: Option<EntryAt>, carve: Option<EntryAt>) -> World<'a> {
        World {
            hide,
            carve,
            ..self
        }
    }

    /// As it is, the columns its queries read reported into `columns`.
    pub fn reading(self, columns: &'a Columns) -> World<'a> {
        World {
            reading: Some(columns),
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

    /// The columns `columns`, read (`reading`): what a kept answer read.
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
pub struct Walked {
    ground: Lines,
    objects: Walk,
}

impl World<'_> {
    /// Where the ground and what is placed stand in the node, as its cells
    /// at `cell` are read; each stepped from where `walked` last went.
    pub fn bounds(&self, walked: &mut Walked, node: Node<2>, cell: Zoom) -> Answer<Option<Aabb>> {
        let objects = self.objects.column(&mut walked.objects, node);
        self.terrain
            .bound_walked(&mut walked.ground, node, cell)
            .map(|ground| ground.into_iter().chain(objects).reduce(Aabb::join))
    }
}
