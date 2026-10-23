// pxl-kernel/query/node.rs

//! What is in a node: the world's one query
//! (doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md,
//! "The query between them"), one record of a node answering two sizes.
//!
//! At the zoom a reader asks, it is what the node is, the block, by the one
//! rule (pxl-spec-core `ground::node`), each source answering from its own
//! level of detail at that zoom and from what the node's parent found,
//! handed down (`At`; `World::block` for an asker with none):
//!
//! - the ground: its column off the windows, as the GPU drew it and the CPU
//!   read it back, and the area's edge laid on it; the ways off the tree the
//!   GPU built of its square, its blocks read back into the pool's mirror,
//!   read as the GPU reads them (pxl-spec-core `window::ground`,
//!   `ways::tree`);
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
//!   block asks `Cell::opens` of one block) over the lowest and highest the windows
//!   hold under the node (`Residency::relief`, off the finest window holding
//!   its cells), every cell under it at each zoom asked over the ground, or
//!   ground a cave may take with a cave through all of it;
//! - the ways: by their own rule (pxl-spec-core `ways::slab`) over their
//!   own bound in the same levels, the leaf cells each column's runs write
//!   joined apart from the ground's (pxl-spec-core fill.rs `level`); a way a
//!   coarser tile drops as narrower than half its cell is dropped there, as
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
use pxl_spec::space::layer::tile::Tile;
use pxl_spec_core::answer::{In, Kind};
use pxl_spec_core::area::Area;
use pxl_spec_core::ground::VOID;
use pxl_spec_core::ground::{self, Cell};
use pxl_spec_core::overwrite::Cut;
use pxl_spec_core::window::{self, Found};
use pxl_spec_core::world;

use super::world::World;
use crate::io::source;
use crate::layer::residency::Lines;
use crate::layer::terrain::window::{pool, Windowed};
use crate::layer::Answer;

/// The coarsest zoom a block is asked at: the coarsest cells a tile draws.
pub(super) fn coarsest() -> Zoom {
    GROUND.grid_zoom(GROUND.top())
}

/// A node as a descent reaches it, X on the asker's lap, and what its
/// ancestors found, handed to its children (pxl-spec-core `world::At`).
#[derive(Clone, Copy)]
pub(super) struct At {
    pub(super) node: Node<3>,
    pub(super) handed: world::At,
}

impl World<'_> {
    /// What is placed as of now: the objects' store's revision, which every
    /// write moves (layer/object/store.rs).
    pub fn revision(&self) -> u32 {
        self.objects.revision()
    }

    /// What is in the block `node`, X on the asker's lap, at its own
    /// zoom, `coarsest` or finer, for an asker with nothing handed down
    /// (`drawn`).
    pub fn block(&self, node: Node<3>) -> Answer<u32> {
        self.drawn(&mut self.start(node), &mut Lines::default())
    }

    /// A node with nothing handed down, a descent's root among them: each
    /// source found down its line from the top.
    pub(super) fn start(&self, node: Node<3>) -> At {
        At {
            node,
            handed: world::At::of(&self.objects.lanes(), node),
        }
    }

    /// What is in `at`'s block, at its own zoom, `coarsest` or finer: none
    /// where it is air or void, else what fills it (`node`, asked at its
    /// own zoom). `lines` are the ground's lines the asker read, which its
    /// own steps from (`node`).
    pub(super) fn drawn(&self, at: &mut At, lines: &mut Lines) -> Answer<u32> {
        let (zoom, _) = at.node;
        xpute_kit::ensure!(zoom >= coarsest(), EINVAL, zoom);
        self.node(at, [zoom, zoom], lines).map(|i| if i.kind == Kind::Block { i.block[0] } else { VOID })
    }

    /// A preview's entry as pxl-spec-core `overwrite` takes it.
    fn cut(&self, entry: Option<EntryAt>) -> Cut {
        entry.map_or(Cut::NONE, |e| Cut { zoom: e.zoom(), at: e.at() })
    }

    /// Its child `k`, in `child_of`'s order, with what it found handed down.
    pub(super) fn child(&self, at: &At, k: usize) -> At {
        let (zoom, c) = at.node;
        let node = (zoom + 1, child_of(c, k));
        At {
            node,
            handed: at.handed.child(&self.objects.lanes(), &pool(), node),
        }
    }

    /// What is in `at`'s node, asked from `far` to `near`, the zooms a reader
    /// asks within it: the world's one query (pxl-spec-core `world::node`),
    /// the ground read as the GPU reads it, off what it drew and read back
    /// (`Windowed`), down the node's line through the residency
    /// (`Reading`). Its answer (pxl-spec-core `answer`): a split and its
    /// slab; a block, the zoom asked's, void for air; or, coarser than asked
    /// and nothing under it filling, air its size, which its zoom against
    /// `asked` tells. The four looks a block is drawn by are the GPU's to
    /// draw. `lines` are the ground's lines the walk asking it read, which
    /// its own steps from (`line_walked`).
    pub(super) fn node(&self, at: &mut At, asked: [Zoom; 2], lines: &mut Lines) -> Answer<In<[u32; 2]>> {
        let (lanes, cuts) = (self.objects.lanes(), [self.cut(self.carve), self.cut(self.hide)]);
        let mut reading = Reading::new(self, lines);
        let i = world::node(&mut reading, &Windowed, &lanes, &pool(), &mut at.handed, at.node, asked, cuts);
        reading.answer(i.kind != Kind::Lacks, || i)
    }
}

/// The ground as a node's reading holds it (pxl-spec-core `ground::Held`):
/// the world's residency down the node's line, the lines its walk read,
/// and the first tile it found lacked or only a coarser one standing in for.
struct Reading<'r, 'a> {
    world: &'r World<'a>,
    lines: &'r mut Lines,
    lacked: Option<Tile>,
    coarser: Option<Tile>,
}

impl<'r, 'a> Reading<'r, 'a> {
    fn new(world: &'r World<'a>, lines: &'r mut Lines) -> Reading<'r, 'a> {
        Reading {
            world,
            lines,
            lacked: None,
            coarser: None,
        }
    }

    /// `of` as the reading went: where `held`, exact, or coarser for the
    /// first tile only a coarser one stood in for; else the lack.
    fn answer<T>(&self, held: bool, of: impl FnOnce() -> T) -> Answer<T> {
        match (held, self.lacked, self.coarser) {
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
                self.lacked = Some(l);
                None
            }
        }
    }
}

impl Area for Reading<'_, '_> {
    fn ends(&self) -> bool {
        source::Declared.ends()
    }
    fn nine(&self, key: u32) -> u32 {
        source::Declared.nine(key)
    }
}

impl ground::Held for Reading<'_, '_> {
    /// The bound over the node of the cells at `cell`, off the windows,
    /// stepped from `place` (pxl-spec-core `Held::relief`); its line names the
    /// tile a lack or a coarser window stands for; where it lacks, `EMPTY`
    /// and false.
    fn relief(&mut self, place: &mut Found, cells: [Zoom; 2], (zoom, c): (Zoom, [i32; 3])) -> (ground::Bounds, bool) {
        let line = self.world.terrain.line_walked(self.lines, (zoom, [c[0], c[1]]), cells[1]);
        match self.take(self.world.terrain.relief(&line, place, (zoom, [c[0], c[1]]), cells)) {
            Some(r) => (r, true),
            None => ([ground::EMPTY; 2], false),
        }
    }

    /// The block's column off the windows, the area's edge laid on it, as
    /// the GPU reads it (pxl-spec-core `window::ground`), stepped from where
    /// its relief found its cells.
    fn cell(&mut self, place: Found, zoom: Zoom, c: [i32; 3]) -> Cell {
        window::ground(&Windowed, place, zoom, [c[0], c[1]]).0
    }
}
