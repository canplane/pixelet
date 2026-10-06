// pxl-kernel/world/access/world.rs

//! The world's answers: the ground's and the placed objects' over it, in one
//! place. Objects are all resident, so what the world knows is what the
//! ground knows.

use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::Node;

use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

use crate::layer::object::store::Cells;

use super::answer::Answer;
use super::ground::Access;
use super::query::{Bound, Query};
use super::terrain::Source;

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

impl Query for Access<'_> {
    type Record<'b>
        = Cube<'b>
    where
        Self: 'b;

    fn bounds(&self, node: Node<3>, cell: Zoom) -> Answer<Option<Bound>> {
        let objects = self.objects.bounds(node, cell).value().flatten();
        self.terrain.bounds(node, cell).map(|ground| join(ground, objects))
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
