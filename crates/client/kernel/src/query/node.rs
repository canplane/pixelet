// pxl-kernel/query/node.rs

//! What is in a node: the world's one query
//! (doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md,
//! "The query between them"), one record of a node answering two sizes.
//!
//! At the zoom a reader asks, it is what the node is, the block, by the one
//! rule (pxl-portable `ground::node`), each source answering from its own
//! level of detail at that zoom and from what the node's parent found,
//! handed down (`At`; `World::block` for an asker with none):
//!
//! - the ground: its column off the windows, as the GPU drew it and the CPU
//!   read it back, and the area's edge laid on it; the ways off the runs the
//!   GPU wrote in its column, read as the GPU reads them (pxl-portable
//!   `window::ground`, `window::ways_over`);
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
//! - the ground: by its one rule (pxl-portable `ground::slab`, as a cell's
//!   block asks `Cell::opens` of one block) over the lowest and highest the windows
//!   hold under the node (`Residency::relief`, off the finest window holding
//!   its cells), every cell under it at each zoom asked over the ground, or
//!   ground a cave may take with a cave through all of it;
//! - the ways: by their own rule (pxl-portable `ways::slab`) over their
//!   own bound in the same levels, the leaf cells each column's runs fill
//!   joined apart from the ground's (pxl-portable fill.rs `level`); a way a
//!   coarser tile drops as narrower than half its cell is dropped there, as
//!   its level of detail does;
//! - the caves: the hash node hollow and drawing nothing again under it
//!   (`procedural::Cave`, one draw of its parent's);
//! - what is placed: no record of the objects' pyramid at the node.
//!
//! A source not holding the node answers that it lacks it, never air and
//! never that nothing fills.

use pxl_portable::area::{Area, Shard};
use pxl_portable::cast::{In, Kind};
use pxl_portable::ground::VOID;
use pxl_portable::ground::{self, Cell};
use pxl_portable::overwrite::{Cut, Held};
use pxl_portable::window::{self, Found};
use pxl_portable::world;
use pxl_spec::bake::constant::EntryAt;
use pxl_spec::procedural::Cave;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::{child_of, Node};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::Tile;

use super::world::World;
use crate::io::source;
use crate::layer::residency::Line;
use crate::layer::terrain::window::Windowed;
use crate::layer::{Answer, Query};

/// The coarsest zoom a block is asked at: the coarsest cells a tile draws.
pub(super) fn coarsest() -> Zoom {
    GROUND.grid_zoom(GROUND.top())
}

/// A node as a descent reaches it, X on the asker's lap, and what its
/// ancestors found, handed to its children.
#[derive(Clone, Copy)]
pub(super) struct At {
    pub(super) node: Node<3>,
    cave: Cave,
    placed: Held,
    /// The ground's line its cell was asked on.
    line: Option<Line>,
    /// What is known of the service area at its shard (pxl-portable
    /// `area::Shard`).
    pub(super) shard: Shard,
    /// Where the windows hold the ground's cells of the nodes over it, which
    /// its own read steps from (pxl-portable `window::step`).
    ground: Found,
}

impl World<'_> {
    /// What is placed as of now: the objects' store's revision, which every
    /// write moves (layer/object/store.rs).
    pub fn revision(&self) -> u32 {
        self.objects.raw_lanes().3
    }

    /// What is in the block `node`, X on the asker's lap, at its own
    /// zoom, `coarsest` or finer, for an asker with nothing handed down
    /// (`drawn`).
    pub fn block(&self, node: Node<3>) -> Answer<u32> {
        self.drawn(&mut self.start(node))
    }

    /// A node with nothing handed down, a descent's root among them: each
    /// source found down its line from the top.
    pub(super) fn start(&self, (zoom, at): Node<3>) -> At {
        At {
            node: (zoom, at),
            cave: Cave::of(zoom, at),
            placed: self.objects.cursor((zoom, at), zoom),
            line: None,
            shard: Shard::UNKNOWN,
            ground: Found::NONE,
        }
    }

    /// The ground's line `at`'s cells at `cell` are read on, stepped from
    /// its parent's, and handed on.
    fn line(&self, at: &mut At, cell: Zoom) -> Line {
        let (zoom, c) = at.node;
        let xy = (zoom, [c[0], c[1]]);
        let line = match at.line {
            Some(l) => self.terrain.child(&l, xy, cell),
            None => self.terrain.cursor(xy, cell),
        };
        at.line = Some(line);
        line
    }

    /// What is in `at`'s block, at its own zoom, `coarsest` or finer: none
    /// where it is air or void, else what fills it (`node`, asked at its
    /// own zoom).
    pub(super) fn drawn(&self, at: &mut At) -> Answer<u32> {
        let (zoom, _) = at.node;
        xpute_kit::ensure!(zoom >= coarsest(), EINVAL, zoom);
        self.node(at, [zoom, zoom]).map(|(i, block)| if block && i.kind == Kind::Block { i.block } else { VOID })
    }

    /// A preview's entry as pxl-portable `overwrite` takes it.
    fn cut(&self, entry: Option<EntryAt>) -> Cut {
        entry.map_or(Cut::NONE, |e| Cut { zoom: e.zoom(), at: e.at() })
    }

    /// Its child `k`, in `child_of`'s order, with what it found handed down.
    pub(super) fn child(&self, at: &At, k: usize) -> At {
        let (zoom, c) = at.node;
        let (z, child) = (zoom + 1, child_of(c, k));
        At {
            node: (z, child),
            cave: at.cave.child(z, child),
            placed: self.objects.child(&at.placed, (z, child), z),
            line: at.line,
            shard: at.shard,
            ground: at.ground,
        }
    }

    /// What is in `at`'s node, asked from `far` to `near`, the zooms a reader
    /// asks within it: the world's one query (pxl-portable `world::node`),
    /// the ground read as the GPU reads it, off what it drew and read back
    /// (`Windowed`), down the node's line through the residency
    /// (`Reading`), after its shard, known before any layer answers it. Its
    /// answer as the descent takes one (pxl-portable `cast::In`): a split and
    /// its slab; a block, the zoom asked's, none for air or void; or, coarser
    /// than asked and nothing under it filling, air its size; with whether it
    /// is a block (`world::node`'s).
    pub(super) fn node(&self, at: &mut At, asked: [Zoom; 2]) -> Answer<(In<u32>, bool)> {
        let (zoom, c) = at.node;
        at.shard = at.shard.at(&Windowed, (zoom, [c[0], c[1]]));
        let (handed, mut place) = ((at.cave, at.placed, at.shard), at.ground);
        let (lanes, cuts) = (self.objects.lanes(), [self.cut(self.carve), self.cut(self.hide)]);
        let mut reading = Reading::new(self, at);
        let (i, block) = world::node(&mut reading, &Windowed, &lanes, handed, &mut place, (zoom, c), asked, cuts);
        let answer = reading.answer(i.kind != Kind::Lacks, || {
            // A block's value, a split's slab (`world::slab_word`); air and void none.
            let b = if block && i.kind == Kind::Block || i.kind == Kind::Split { i.block } else { VOID };
            (In { kind: i.kind, block: b }, block)
        });
        at.ground = place;
        answer
    }
}

/// The ground as a node's reading holds it (pxl-portable `ground::Held`):
/// the world's residency down the node's line, and the first tile it found
/// lacked or only a coarser one standing in for.
struct Reading<'r, 'a> {
    world: &'r World<'a>,
    at: &'r mut At,
    lacked: Option<Tile>,
    coarser: Option<Tile>,
}

impl<'r, 'a> Reading<'r, 'a> {
    fn new(world: &'r World<'a>, at: &'r mut At) -> Reading<'r, 'a> {
        Reading {
            world,
            at,
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
    fn entries(&self) -> u32 {
        source::Declared.entries()
    }
    fn entry(&self, i: u32) -> u32 {
        source::Declared.entry(i)
    }
}

impl ground::Held for Reading<'_, '_> {
    /// The bound over the node of the cells at `cell`, off the windows,
    /// stepped from `place` (pxl-portable `Held::relief`); its line names the
    /// tile a lack or a coarser window stands for; where it lacks, `EMPTY`
    /// and false.
    fn relief(&mut self, place: &mut Found, cells: [Zoom; 2], _: (Zoom, [i32; 3])) -> (ground::Bounds, bool) {
        let (zoom, c) = self.at.node;
        let line = self.world.line(self.at, cells[1]);
        match self.take(self.world.terrain.relief(&line, place, (zoom, [c[0], c[1]]), cells)) {
            Some(r) => (r, true),
            None => ([ground::EMPTY; 2], false),
        }
    }

    /// The block's column off the windows, the area's edge laid on it, as
    /// the GPU reads it (pxl-portable `window::ground`), stepped from where
    /// its relief found its cells.
    fn cell(&mut self, place: Found, zoom: Zoom, c: [i32; 3]) -> Cell {
        window::ground(&Windowed, (zoom, [c[0], c[1]]), place, zoom, [c[0], c[1]]).0
    }
}
