// pxl-kernel/query/world.rs

//! The world's answers: the ground's and the placed objects' over it, in one
//! place. Objects are all resident, so what the world knows is what the
//! ground knows.

use crate::layer::residency::Line;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::Node;

use super::block::Column;
use crate::layer::object::store::EntryAt;

use super::decor::Grown;
use crate::kernel;
use crate::layer::object::store::Store;
use crate::layer::residency::Residency;
use crate::layer::Answer;
use crate::layer::Base as _;
use crate::layer::{Bound, Query};
use pxl_spec::bake::format::TerrainTile;
use pxl_spec::space::camera::grid::Tau;

/// The world's query: the resident ground and the placed objects, read
/// as one.
#[derive(Clone, Copy)]
pub struct World<'a> {
    pub(super) terrain: &'a Residency<TerrainTile>,
    pub(super) objects: &'a Store,
    /// How decor is struck; none, and a ray passes through decor.
    pub(super) decor: Option<Grown<'a>>,
    pub(super) visits: Option<&'a std::cell::Cell<u64>>,
    /// What of what is placed a preview hides, and what it cuts to air.
    pub(super) hide: Option<EntryAt>,
    pub(super) carve: Option<EntryAt>,
}

impl World<'static> {
    pub fn now() -> World<'static> {
        let objects: &'static Store = &kernel().objects;
        World::new(TerrainTile::residency(), objects)
    }

    /// As `now`, and decor is struck too, where the chunks drawing at `tau`
    /// hold it: the pointer's pick, which alone asks what a decor thing is.
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
            visits: None,
            hide: None,
            carve: None,
        }
    }

    /// As a preview shows it: `hide` left out, `carve` cut to air.
    pub fn previewing(self, hide: Option<EntryAt>, carve: Option<EntryAt>) -> World<'a> {
        World { hide, carve, ..self }
    }

    pub fn counting(self, visits: &'a std::cell::Cell<u64>) -> World<'a> {
        World { visits: Some(visits), ..self }
    }

    pub(super) fn visit(&self) {
        if let Some(v) = self.visits {
            v.set(v.get() + 1);
        }
    }
}

fn join(a: Option<Bound>, b: Option<Bound>) -> Option<Bound> {
    match (a, b) {
        (Some(a), Some(b)) => Some(Bound {
            aabb: a.aabb.join(b.aabb),
            finest: a.finest.max(b.finest),
        }),
        (a, b) => a.or(b),
    }
}

impl Query for World<'_> {
    type Record<'b>
        = Column<'b>
    where
        Self: 'b;

    type Cursor = Line;

    fn cursor<const D: usize>(&self, node: Node<D>, cell: Zoom) -> Line {
        self.terrain.cursor(node, cell)
    }

    fn child<const D: usize>(&self, parent: &Line, node: Node<D>, cell: Zoom) -> Line {
        self.terrain.child(parent, node, cell)
    }

    fn bound<const D: usize>(&self, at: &Line, node: Node<D>, cell: Zoom) -> Answer<Option<Bound>> {
        let objects = self.objects.bounds(node, cell).value().flatten();
        self.terrain.bound(at, node, cell).map(|ground| join(ground, objects))
    }

    /// A chunk column's records to compose its blocks from.
    fn read(&self, node: Node<3>, cell: Zoom) -> Answer<Column<'_>> {
        self.column(node, cell)
    }
}
