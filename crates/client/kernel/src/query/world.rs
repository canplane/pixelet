// pxl-kernel/query/world.rs

//! The world's answers: the ground's and the placed objects' over it, in one
//! place. Objects are all resident, so what the world knows is what the
//! ground knows.

use crate::layer::residency::Line;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::Node;

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::layer::object::store::Cells;

use super::decor::Grown;
use crate::kernel;
use crate::layer::object::store::{Store, AIR};
use crate::layer::residency::Residency;
use crate::layer::terrain::Source;
use crate::layer::Answer;
use crate::layer::Base as _;
use crate::layer::{Bound, Query};
use pxl_spec::bake::format::TerrainTile;
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::layer::lattice::GROUND;

/// The world's query: the resident ground and the placed objects, read
/// as one.
#[derive(Clone, Copy)]
pub struct World<'a> {
    pub(super) terrain: &'a Residency<TerrainTile>,
    pub(super) objects: &'a Store,
    /// How decor is struck; none, and a ray passes through decor.
    pub(super) decor: Option<Grown<'a>>,
    pub(super) visits: Option<&'a std::cell::Cell<u64>>,
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
        }
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
pub struct Composed<'a> {
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
        = Composed<'b>
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

    fn read(&self, node: Node<3>, cell: Zoom) -> Answer<Composed<'_>> {
        let objects = self.objects.read(node, cell).value();
        self.terrain.read(node, cell).map(|ground| Composed {
            ground,
            // Objects are all resident: their answer is always exact.
            objects: objects.or_bug(Errno::ENOTRECOVERABLE),
        })
    }
}

/// What the objects hold where the ground grows a thing, as the composer
/// reads them: in the cell it stands in and the cell under, its footing; at a
/// leaf's cells whether anything is held, over a fold what it folds to.
pub enum Cover {
    Leaf { here: bool, under: bool },
    Folded { here: Option<u32>, under: Option<u32> },
}

impl Cover {
    /// At the leaf cell `cell`, in the store.
    pub fn leaf(store: &Store, [x, y, z]: [i64; 3]) -> Cover {
        let held = |z: i64| store.read((GROUND.cell(), [x, y, z]), GROUND.cell()).value().is_some_and(|c| c.holds());
        Cover::Leaf { here: held(z), under: held(z - 1) }
    }
}

/// Whether the objects cover what the ground grows in a cell, so it is
/// neither drawn nor struck: anything held under it takes its footing, air
/// dug there included; in its own cell, at a leaf's cells anything held does,
/// air included, so erasing a grown tree is placing air over it, and over a
/// fold only a fill does.
pub fn covers(cover: Cover) -> bool {
    match cover {
        Cover::Leaf { here, under } => here || under,
        Cover::Folded { here, under } => here.is_some_and(|s| s != AIR) || under.is_some(),
    }
}
