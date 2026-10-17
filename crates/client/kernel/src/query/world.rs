// pxl-kernel/query/world.rs

//! The world's answers: the ground's and the placed objects' over it, in one
//! place. Objects are all resident, so what the world knows is what the
//! ground knows.

use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::Node;

use crate::layer::object::store::EntryAt;

use super::decor::Grown;
use crate::kernel;
use crate::layer::object::store::Store;
use crate::layer::residency::{Lines, Residency};
use crate::layer::Answer;
use crate::layer::Base as _;
use crate::layer::{Bound, Query};
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::camera::grid::Tau;

/// The world's query: the resident ground and the placed objects, read
/// as one.
#[derive(Clone, Copy)]
pub struct World<'a> {
    pub(super) terrain: &'a Residency<TerrainTile>,
    pub(super) objects: &'a Store,
    /// How decor is struck; none, and a ray passes through decor.
    pub(super) decor: Option<Grown<'a>>,
    /// What of what is placed a preview hides, and what it cuts to air.
    pub(super) hide: Option<EntryAt>,
    pub(super) carve: Option<EntryAt>,
}

impl World<'static> {
    /// The world as stored: what every mechanic asks, the pick, a walk, the
    /// camera's clearance, an edit's reads and decor's footing.
    pub fn now() -> World<'static> {
        let objects: &'static Store = &kernel().objects;
        World::new(TerrainTile::residency(), objects)
    }

    /// As `now`, and decor is struck too, where the frame drawing at `tau`
    /// holds it: the pointer's pick, which alone asks what a decor thing is.
    pub fn striking_decor(self, tau: Tau) -> World<'static> {
        let decor = Grown { models: &kernel().models, tau };
        World { decor: Some(decor), ..self }
    }
}

impl<'a> World<'a> {
    pub fn new(terrain: &'a Residency<TerrainTile>, objects: &'a Store) -> World<'a> {
        World {
            terrain,
            objects,
            decor: None,
            hide: None,
            carve: None,
        }
    }

    /// As a preview shows it: `hide` left out, `carve` cut to air, at the
    /// entry's zoom or finer, where it is drawn as held; coarser, the stored
    /// fold stands.
    pub fn previewing(self, hide: Option<EntryAt>, carve: Option<EntryAt>) -> World<'a> {
        World { hide, carve, ..self }
    }
}

impl World<'_> {
    /// Where the ground and what is placed stand in the node, as its cells
    /// at `cell` are read; the ground's line stepped from where `lines` last
    /// walked.
    pub fn bounds(&self, lines: &mut Lines, node: Node<2>, cell: Zoom) -> Answer<Option<Bound>> {
        let objects = self.objects.bounds(node, cell).value().flatten();
        self.terrain.bound_walked(lines, node, cell).map(|ground| Bound::join(ground, objects))
    }
}
