// pxl-kernel/query/world.rs

//! The world's answers: the ground's and the placed objects' over it, in one
//! place. Objects are all resident, so what the world knows is what the
//! ground knows.

use crate::layer::residency::Line;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::Node;

use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

use crate::layer::object::store::Cells;

use crate::kernel;
use crate::layer::object::store::Store;
use crate::layer::residency::Residency;
use crate::layer::terrain::Source;
use crate::layer::Answer;
use crate::layer::Layer as _;
use crate::layer::{Bound, Query};
use pxl_spec::bake::format::Terrain;

/// The world's query: the resident ground and the placed objects, read
/// as one.
#[derive(Clone, Copy)]
pub struct World<'a> {
    pub(super) terrain: &'a Residency<Terrain>,
    pub(super) objects: &'a Store,
    pub(super) visits: Option<&'a std::cell::Cell<u64>>,
}

impl World<'static> {
    pub fn now() -> World<'static> {
        let objects: &'static Store = &kernel().objects;
        World::new(Terrain::residency(), objects)
    }
}

impl<'a> World<'a> {
    pub fn new(terrain: &'a Residency<Terrain>, objects: &'a Store) -> World<'a> {
        World { terrain, objects, visits: None }
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

/// A node's cells as the world holds them: the ground they stand on and the
/// objects placed in them.
#[derive(Clone, Copy)]
pub struct Cube<'a> {
    pub ground: Source<'a>,
    pub objects: Cells<'a>,
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
        = Cube<'b>
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

    fn read(&self, node: Node<3>, cell: Zoom) -> Answer<Cube<'_>> {
        let objects = self.objects.read(node, cell).value();
        self.terrain.read(node, cell).map(|ground| Cube {
            ground,
            // Objects are all resident: their answer is always exact.
            objects: objects.or_bug(Errno::ENOTRECOVERABLE),
        })
    }
}
