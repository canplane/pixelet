// pxl-kernel/domain/query/node.rs

//! What is in a node: the world's one query, one record of a node answering
//! two sizes.
//!
//! At the zoom a reader asks, it is what the node is, the cell, by the one
//! rule (pxl-spec-core `ground::node`), each source answering from its own
//! level of detail at that zoom and from what the node's parent found,
//! handed down (`At`; `World::asset` for an asker with none):
//!
//! - the ground: its column off the clip levels, as the GPU drew it and the CPU
//!   read it back, the wall where it is the wall; the ways off the tree the
//!   GPU built of its brick, its child groups read back into the pool's mirror,
//!   read as the GPU reads them (pxl-spec-core `clipmap::ground`,
//!   `way::tree`);
//! - the caves: the hash pyramid's node at that zoom, its value
//!   (`procedural.rs` `Cave`);
//! - what is placed: the objects' pyramid record at that zoom, folded by
//!   their majority (`pyramid.rs`, through the store's query).
//!
//! Coarser, it is whether anything under the node fills at the zooms a
//! reader asks within it, the world as that level of detail defines it and
//! nothing finer: what those zooms fold away is not there to miss
//! (`World::node`). Each source answers from what it already keeps and what
//! the node's parent found, handed down, as surely or more cautiously, which
//! only splits more, never from detail finer than asked:
//!
//! - the ground: by its one rule (pxl-spec-core `ground::slab`, as a cell's
//!   asset asks `Cell::opens` of one cell) over the lowest and highest the
//!   clip levels hold under the node (`TileCache::relief`, off the finest clip
//!   level holding its cells), every cell under it at each zoom asked over the
//!   ground, or ground a cave may take with a cave through all of it;
//! - the ways: by their own rule (pxl-spec-core `way::slab`) over their
//!   own bound in the same levels, the leaf cells the ways write in each
//!   column (pxl-spec-core `way::lined_hull`) joined apart from the ground's
//!   (pxl-spec-core fill.rs `level`); a way a coarser tile drops as
//!   narrower than half its cell is dropped there, as
//!   its level of detail does;
//! - the caves: the hash node hollow and drawing nothing again under it
//!   (`procedural::Cave`, one draw of its parent's);
//! - what is placed: no record of the objects' pyramid at the node.
//!
//! A source not holding the node answers that it lacks it, never air and
//! never that nothing fills.

use pxl_spec::bake::constant::EntryAt;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::{child_of, Node};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{Tile, TOP_ZOOM};
use pxl_spec_core::answer::{In, Kind};
use pxl_spec_core::area::{Area, Shard};
use pxl_spec_core::clipmap::{self, Lookup};
use pxl_spec_core::ground::VOID;
use pxl_spec_core::ground::{self, Cell};
use pxl_spec_core::overwrite::Cut;
use pxl_spec_core::world;

use super::world::World;
use crate::domain::layer::terrain::clipmap::{pool, CpuClipmap};
use crate::domain::layer::tile_cache::Lines;
use crate::domain::layer::Answer;
use crate::sys::io::source;

/// The coarsest zoom a cell is asked at: the coarsest cells a tile draws.
pub(super) fn coarsest() -> Zoom {
    GROUND.grid_zoom(TOP_ZOOM)
}

/// A node as a descent reaches it, X on the asker's lap, and what its
/// ancestors found, handed to its children (pxl-spec-core `world::Inheritance`).
#[derive(Clone, Copy)]
pub(super) struct At {
    pub(super) node: Node<3>,
    pub(super) inheritance: world::Inheritance,
}

impl World<'_> {
    /// What is placed as of now: the objects' store's revision, which every
    /// write moves (domain/layer/object/store.rs).
    pub fn revision(&self) -> u32 {
        self.object.revision()
    }

    /// What is in the cell `node`, X on the asker's lap, at its own
    /// zoom, `coarsest` or finer, for an asker with nothing handed down
    /// (`asset_at`).
    pub fn asset(&self, node: Node<3>) -> Answer<u32> {
        self.asset_at(&mut self.start(node), &mut Lines::default())
    }

    /// A node with nothing handed down, a descent's root among them: each
    /// source found down its line from the top.
    pub(super) fn start(&self, node: Node<3>) -> At {
        At {
            node,
            inheritance: if self.unplaced {
                world::Inheritance::unplaced(node)
            } else {
                world::Inheritance::of(&self.object.columns(), node)
            },
        }
    }

    /// What is in `at`'s cell, at its own zoom, `coarsest` or finer: none
    /// where it is air or void, else what fills it (`node`, asked at its
    /// own zoom). `lines` are the ground's lines the asker read, which its
    /// own steps from (`node`).
    pub(super) fn asset_at(&self, at: &mut At, lines: &mut Lines) -> Answer<u32> {
        let (zoom, _) = at.node;
        xpute_kit::ensure!(zoom >= coarsest(), EINVAL, zoom);
        self.node(at, [zoom, zoom], lines).map(|i| {
            if i.kind == Kind::Cell {
                i.asset[0]
            } else {
                VOID
            }
        })
    }

    /// A preview's entry as pxl-spec-core `overwrite` takes it.
    fn cut(&self, entry: Option<EntryAt>) -> Cut {
        entry.map_or(Cut::NONE, |e| Cut {
            zoom: e.zoom(),
            at: e.at(),
        })
    }

    /// Its child `k`, in `child_of`'s order, with what it found handed down.
    pub(super) fn child(&self, at: &At, k: usize) -> At {
        let (zoom, c) = at.node;
        let node = (zoom + 1, child_of(c, k));
        At {
            node,
            inheritance: at.inheritance.child(&self.object.columns(), pool(), node),
        }
    }

    /// What is in `at`'s node, asked from `far` to `near`, the zooms a reader
    /// asks within it: the world's one query (pxl-spec-core `world::node`),
    /// the ground read as the GPU reads it, off what it drew and read back
    /// (`CpuClipmap`), down the node's line through the tile cache
    /// (`Reading`). Its answer (pxl-spec-core `answer`): a split and its
    /// box in its own steps; the asset of a cell at the zoom asked, void for air; or,
    /// coarser than asked and nothing under it filling, air its size, which
    /// its zoom against `asked` tells. The four palette indices a cell is drawn by are the GPU's to
    /// draw. `lines` are the ground's lines the walk asking it read, which
    /// its own steps from (`walk_line`).
    pub(super) fn node(
        &self,
        at: &mut At,
        asked: [Zoom; 2],
        lines: &mut Lines,
    ) -> Answer<In<[u32; 2]>> {
        let (lanes, cuts) = (
            self.object.columns(),
            [self.cut(self.carve), self.cut(self.hide)],
        );
        let mut reading = Reading::new(self, lines);
        let i = world::node(
            &mut reading,
            &CpuClipmap,
            &lanes,
            pool(),
            &mut at.inheritance,
            at.node,
            asked,
            cuts,
        );
        reading.answer(i.kind != Kind::Lacks, || i)
    }
}

/// The ground as a node's reading holds it (pxl-spec-core `ground::Resident`):
/// the world's tile cache down the node's line, the lines its walk read,
/// and the first tile it found lacked or only a coarser one standing in for.
struct Reading<'r, 'a> {
    world: &'r World<'a>,
    lines: &'r mut Lines,
    missing: Option<Tile>,
    coarser: Option<Tile>,
}

impl<'r, 'a> Reading<'r, 'a> {
    fn new(world: &'r World<'a>, lines: &'r mut Lines) -> Reading<'r, 'a> {
        Reading {
            world,
            lines,
            missing: None,
            coarser: None,
        }
    }

    /// `of` as the reading went: where `held`, exact, or coarser for the
    /// first tile only a coarser one stood in for; else the lack.
    fn answer<T>(&self, held: bool, of: impl FnOnce() -> T) -> Answer<T> {
        match (held, self.missing, self.coarser) {
            (false, Some(l), _) => Answer::Nothing(l),
            (_, _, Some(l)) => Answer::Coarser(of(), l),
            _ => Answer::Exact(of()),
        }
    }

    /// `answer`'s value, noting its lack.
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

impl Area for Reading<'_, '_> {
    fn ends(&self) -> bool {
        source::ServiceArea.ends()
    }
    fn nine(&self, key: u32) -> u32 {
        source::ServiceArea.nine(key)
    }
}

impl ground::Resident for Reading<'_, '_> {
    /// The bound over the node of the cells at `cell`, off the clip levels,
    /// stepped from `place` (pxl-spec-core `ground::Resident::relief`); its line names the
    /// tile a lack or a coarser clip level stands for; where it lacks, `EMPTY`
    /// and false.
    fn relief(
        &mut self,
        place: &mut Lookup,
        cells: [Zoom; 2],
        (zoom, c): (Zoom, [i32; 3]),
    ) -> (ground::Bounds, bool) {
        let line = self
            .world
            .terrain
            .walk_line(self.lines, (zoom, [c[0], c[1]]), cells[1]);
        match self.take(
            self.world
                .terrain
                .relief(&line, place, (zoom, [c[0], c[1]]), cells),
        ) {
            Some(r) => (r, true),
            None => ([ground::EMPTY; 2], false),
        }
    }

    /// The cell's column off the clip levels, the wall where it is the
    /// wall, as the GPU reads it (pxl-spec-core `clipmap::ground`), stepped
    /// from where its relief found its cells.
    fn cell(&mut self, shard: Shard, place: Lookup, zoom: Zoom, c: [i32; 3]) -> Cell {
        clipmap::ground(&CpuClipmap, shard, place, zoom, [c[0], c[1]]).0
    }
}
