// pxl-kernel/query/world.rs

//! The world's answers: the ground's and the placed objects' over it, in one
//! place. Objects are all resident, so what the world knows is what the
//! ground knows.

use crate::layer::residency::Line;
use pxl_spec::block::limit::floor;
use pxl_spec::procedural::{opened_heights, CAVE_REGION};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::{ancestor_of, region, Node};
use pxl_spec::space::layer::lattice::GROUND;

use super::block::Column;
use crate::layer::object::store::EntryAt;

use super::decor::Grown;
use crate::kernel;
use crate::layer::object::store::Store;
use crate::layer::residency::Residency;
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

    /// The ground's, what is placed, and the caves opened about it, which a
    /// chunk finer under the node may draw at any zoom it is asked at.
    fn bound<const D: usize>(&self, at: &Line, node: Node<D>, cell: Zoom) -> Answer<Option<Bound>> {
        let objects = self.objects.bounds(node, cell).value().flatten();
        let caves = self.caves(node, cell);
        self.terrain.bound(at, node, cell).map(|ground| join(join(ground, objects), caves))
    }

    /// A chunk column's records to compose its blocks from.
    fn read(&self, node: Node<3>, cell: Zoom) -> Answer<Column<'_>> {
        self.column(node, cell)
    }
}

impl World<'_> {
    /// Where caves may be carved under `node`: over its columns, between the
    /// ground's floor and face, at the heights opened about what is placed in
    /// its cube columns or one beside (`pxl-spec procedural.rs`
    /// `opened_heights`), the rule the composition opens them by.
    fn caves<const D: usize>(&self, (zoom, at): Node<D>, cell: Zoom) -> Option<Bound> {
        let side = zoom.min(CAVE_REGION);
        let a = ancestor_of([at[0], at[1]], u32::from(zoom - side));
        let mut edits: Option<Aabb> = None;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if let Some(b) = self.objects.bounds((side, [a[0] + dx, a[1] + dy]), cell).value().flatten() {
                    edits = Some(edits.map_or(b.aabb, |e| e.join(b.aabb)));
                }
            }
        }
        let [lo, hi] = opened_heights([edits?.min[2], edits?.max[2]]);
        let ground = self.terrain.bounds((zoom, [at[0], at[1]]), cell).value().flatten()?.aabb;
        let mut b = region((zoom, at));
        (b.min[2], b.max[2]) = (b.min[2].max(lo).max(floor(ground.min[2])), b.max[2].min(hi).min(ground.max[2]));
        (b.min[2] < b.max[2]).then_some(Bound { aabb: b, finest: GROUND.cell() })
    }
}
