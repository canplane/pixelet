// pxl-core/domain/query/node.rs

//! What is in a node: the world's one query. One record of a node answers
//! for two sizes, the cell at the zoom asked and a coarser node.
//!
//! At the zoom a reader asks, the answer is what the node is: the cell, by
//! the one rule (pxl-engine `ground::node`). Each source answers from its
//! own level of detail at that zoom and from what the node's parent found
//! and handed down (`At`; `World::asset` serves an asker with nothing handed
//! down):
//!
//! - the ground: its column off the clip levels, as the GPU reads it, or the
//!   wall at the service area's edge where the column is that wall. The ways
//!   are read off the spans their brick's fill kept, which lie in the pool
//!   and are read as the GPU reads them (pxl-engine `clipmap::ground`,
//!   `clipmap::way_at`);
//! - the caves: the noises at the cell's center (pxl-content `cave::hollow`);
//! - what is placed: the objects' pyramid record at that zoom, folded by
//!   majority (domain/layer/object/store.rs, through the store's query).
//!
//! At a coarser zoom, the answer is whether anything under the node fills at
//! the zooms the reader asks within it. The world there is what that level
//! of detail defines and nothing finer: what those zooms fold away is not
//! there to miss (`World::node`). Each source answers from what it already
//! keeps and from what the node's parent found, as surely or more
//! cautiously, which can only split the node more. No source reads detail
//! finer than asked:
//!
//! - the ground: by its one rule (pxl-engine `ground::slab`, which a cell's
//!   asset applies to one cell as `Cell::opens`) over the lowest and highest
//!   heights the clip levels hold under the node (`TileCache::relief`, off
//!   the finest clip level holding its cells). Every cell under the node at
//!   each zoom is asked against the ground, or against the ground a cave may
//!   take where a cave runs through all of the node;
//! - the ways: by their own rule (pxl-engine `way::slab`) over their own
//!   bound in the same levels. That bound is the leaf cells the ways write in
//!   each column (pxl-engine `way::lined_hull`), joined apart from the
//!   ground's (pxl-engine fill.rs `mip`). A way that a coarser tile drops as
//!   narrower than half its cell is dropped there too, as that level of
//!   detail does;
//! - the caves: whether a cave runs through all of the node, by the noises'
//!   least and most over it (pxl-content `cave::all_hollow`);
//! - what is placed: whether the objects' pyramid has no record at the node.
//!
//! A source that does not hold the node answers that it lacks it, never air
//! and never that nothing fills.

use pxl_content::Pixelet;
use pxl_engine::answer::{In, Kind};
use pxl_engine::area::{Area, Shard};
use pxl_engine::clipmap::{self, Lookup};
use pxl_engine::ground::VOID;
use pxl_engine::ground::{self, Cell};
use pxl_engine::object::{child_at, Cut};
use pxl_engine::world;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::octant::Octant;
use pxl_spec::space::layer::quadrant::{Quadrant, MIN_ZOOM};

use super::world::World;
use crate::domain::layer::terrain::clipmap::{pool, CpuClipmap};
use crate::domain::layer::tile_cache::Path;
use crate::domain::layer::Answer;
use crate::sys::io::source;

/// The coarsest zoom a cell is asked at: that of the coarsest cells a tile
/// draws.
pub(super) fn coarsest() -> Zoom {
    GROUND.grid_zoom(MIN_ZOOM)
}

/// A node as a descent reaches it, with X on the asker's lap, and what its
/// ancestors found, which it hands to its children (pxl-engine
/// `world::Inheritance`).
#[derive(Clone, Copy)]
pub(super) struct At {
    pub(super) node: Node<3>,
    pub(super) inheritance: world::Inheritance,
}

impl World<'_> {
    /// The object store's revision, which every write moves
    /// (domain/layer/object/store.rs). Equal revisions mean the same things
    /// are placed.
    pub fn revision(&self) -> u32 {
        self.object.revision()
    }

    /// What is in the cell `node` at its own zoom, which is `coarsest` or
    /// finer, for an asker with nothing handed down (`asset_at`). X is on the
    /// asker's lap.
    pub fn asset(&self, node: Node<3>) -> Answer<u32> {
        self.asset_at(&mut self.start(node), &mut Path::default())
    }

    /// `node` with nothing handed down, as a descent's root is. Each source
    /// finds what it needs down the node's line from the top.
    pub(super) fn start(&self, node: Node<3>) -> At {
        At {
            node,
            inheritance: if self.unplaced {
                world::Inheritance::UNPLACED
            } else {
                world::Inheritance::of(&self.object.columns(), node)
            },
        }
    }

    /// What is in `at`'s cell at its own zoom, which is `coarsest` or finer:
    /// what fills it (`node`, asked at that zoom), or void where it is air or
    /// void. `path` are the ground's lines the asker read, and the cell's
    /// line steps on from them (`node`).
    pub(super) fn asset_at(&self, at: &mut At, path: &mut Path) -> Answer<u32> {
        let (zoom, _) = at.node;
        xpute_kit::ensure!(zoom >= coarsest(), EINVAL, zoom);
        self.node(at, [zoom, zoom], path).map(|i| {
            if i.kind == Kind::Cell {
                i.asset[0]
            } else {
                VOID
            }
        })
    }

    /// A preview's entry in the form pxl-engine `object` takes.
    fn cut(&self, entry: Option<Octant>) -> Cut {
        entry.map_or(Cut::NONE, |e| Cut {
            zoom: e.zoom(),
            at: e.at(),
        })
    }

    /// `at`'s child `k`, in `child_of`'s order, with what `at` found handed
    /// down to it.
    pub(super) fn child(&self, at: &At, k: usize) -> At {
        let (zoom, c) = at.node;
        let node = (zoom + 1, child_at(zoom, c, k as u32));
        At {
            node,
            inheritance: at.inheritance.child(&self.object.columns(), node),
        }
    }

    /// What is in `at`'s node, asked over `zooms`, the zooms from far to
    /// near that a reader asks within it. This is the world's one query
    /// (pxl-engine `world::node`). The ground is read as the GPU reads it,
    /// off the clip levels (`CpuClipmap`), down the node's line through the
    /// tile cache (`GroundRead`).
    ///
    /// The answer (pxl-engine `answer`) is one of three: a split, with its
    /// box in its own steps; the asset of a cell at the zoom asked, void for
    /// air; or, for a node coarser than asked with nothing under it filling,
    /// air of the node's size, which its zoom against `zooms` tells apart.
    /// The four palette indices a cell is drawn by are left to the GPU.
    /// `path` are the ground's lines the asking walk read, and the node's
    /// line steps on from them (`walk_line`).
    pub(super) fn node(
        &self,
        at: &mut At,
        zooms: [Zoom; 2],
        path: &mut Path,
    ) -> Answer<In<[u32; 2]>> {
        let (lanes, cuts) = (
            self.object.columns(),
            [self.cut(self.carve), self.cut(self.hide)],
        );
        let mut reading = GroundRead::new(self, path);
        let i = world::node(
            &mut reading,
            &CpuClipmap,
            &lanes,
            pool(),
            &mut at.inheritance,
            at.node,
            zooms,
            cuts,
        );
        reading.answer(i.kind != Kind::Lacks, || i)
    }
}

/// The ground as a node's reading holds it (pxl-engine `ground::Resident`):
/// the world's tile cache down the node's line, the lines its walk read, and
/// the first tile the reading found missing (`missing`) or held only through
/// a coarser tile standing in for it (`coarser`).
struct GroundRead<'r, 'a> {
    world: &'r World<'a>,
    path: &'r mut Path,
    missing: Option<Quadrant>,
    coarser: Option<Quadrant>,
}

impl<'r, 'a> GroundRead<'r, 'a> {
    fn new(world: &'r World<'a>, path: &'r mut Path) -> GroundRead<'r, 'a> {
        GroundRead {
            world,
            path,
            missing: None,
            coarser: None,
        }
    }

    /// `of` as the reading went. Where `found`, it is exact, or coarser with
    /// the first tile that only a coarser one stood in for. Otherwise the
    /// answer is the missing tile.
    fn answer<T>(&self, found: bool, of: impl FnOnce() -> T) -> Answer<T> {
        match (found, self.missing, self.coarser) {
            (false, Some(l), _) => Answer::Nothing(l),
            (_, _, Some(l)) => Answer::Coarser(of(), l),
            _ => Answer::Exact(of()),
        }
    }

    /// The value of `a`, noting for `answer` the tile it lacked.
    fn take<T>(&mut self, a: Answer<T>) -> Option<T> {
        match a {
            Answer::Exact(v) => Some(v),
            Answer::Coarser(v, l) => {
                self.coarser.get_or_insert(l);
                Some(v)
            }
            Answer::Nothing(l) => {
                self.missing = Some(l);
                None
            }
        }
    }
}

impl Area for GroundRead<'_, '_> {
    fn ends(&self) -> bool {
        source::ServiceArea.ends()
    }
    fn nine(&self, key: u32) -> u32 {
        source::ServiceArea.nine(key)
    }
}

impl ground::Resident for GroundRead<'_, '_> {
    type Content = Pixelet;
    /// The bound over the node of its cells at the zooms `cells`, off the
    /// clip levels, stepped from `place` (pxl-engine
    /// `ground::Resident::relief`). The node's line names the tile that a
    /// lack or a coarser clip level stands for. Where the bound is lacking,
    /// it is `EMPTY` with false.
    fn relief(
        &mut self,
        place: &mut Lookup,
        cells: [Zoom; 2],
        (zoom, c): (Zoom, [i32; 3]),
    ) -> (ground::Extent, bool) {
        let line = self
            .world
            .terrain
            .walk_line(self.path, (zoom, [c[0], c[1]]), cells[1]);
        match self.take(
            self.world
                .terrain
                .relief(&line, place, (zoom, [c[0], c[1]]), cells),
        ) {
            Some(r) => (r, true),
            None => ([ground::EMPTY; 2], false),
        }
    }

    /// The cell's column off the clip levels as the GPU reads it (pxl-engine
    /// `clipmap::ground`), or the wall where the column is the wall. The
    /// lookup steps on from where the node's relief found its cells.
    fn cell(&mut self, shard: Shard, place: Lookup, zoom: Zoom, c: [i32; 3]) -> Cell {
        clipmap::ground(&CpuClipmap, shard, place, zoom, [c[0], c[1]]).0
    }
}
