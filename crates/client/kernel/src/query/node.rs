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
//!   GPU wrote in its column and those about it, read as the GPU reads them
//!   (pxl-portable `window::ground`, `window::ways_over`);
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
//! - the ground: by its one rule (pxl-portable `ground::empty`, as a cell's
//!   block asks `Cell::opens` of one block) over the lowest and highest the windows
//!   hold under the node (`Residency::relief`, off the finest window holding
//!   its cells), every cell under it at each zoom asked over the ground, or
//!   ground a cave may take with a cave through all of it;
//! - the ways: the same levels, which join each bin's ways' bound with the
//!   ground's (pxl-portable `window`); a way a coarser tile drops as
//!   narrower than half its cell is dropped there, as its level of detail
//!   does;
//! - the caves: the hash node hollow and drawing nothing again under it
//!   (`procedural::Cave`, one draw of its parent's);
//! - what is placed: no record of the objects' pyramid at the node.
//!
//! A source not holding the node answers that it lacks it, never air and
//! never that nothing fills.

use pxl_portable::area::{Area, Shard};
use pxl_portable::cast::Kind;
use pxl_portable::cube::sentinel;
use pxl_portable::ground::{self, Cell};
use pxl_portable::window::{self, Found};
use pxl_spec::bake::constant::EntryAt;
use pxl_spec::bake::tile::terrain::GroundId;
use pxl_spec::block::placed::{self, Cut, Held, BLOCK, SPLIT};
use pxl_spec::block::Block;
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

/// What is in a node, at the zoom asked or over it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum In {
    /// Coarser than asked: nothing under it fills at the zooms asked.
    Empty,
    /// Coarser than asked: something under it may fill.
    Split,
    /// At the zoom asked: the block, none for air or void.
    Block(Option<Block>),
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
    shard: Shard,
    /// Where the windows hold the ground's cells of the nodes over it, which
    /// its own read steps from (pxl-portable `window::step`).
    ground: Found,
}

impl World<'_> {
    /// What is in the block `b` of `zoom`, X on the asker's lap, at its own
    /// zoom, `coarsest` or finer, for an asker with nothing handed down
    /// (`drawn`).
    pub fn block(&self, node: Node<3>) -> Answer<Option<Block>> {
        self.drawn(&mut self.start(node))
    }

    /// A node with nothing handed down, a descent's root among them: each
    /// source found down its line from the top.
    pub(super) fn start(&self, (zoom, at): Node<3>) -> At {
        At {
            node: (zoom, at),
            cave: Cave::of(zoom, at.map(|v| v as i32)),
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
    /// where it is air or void, else what fills it or not, the layers read
    /// top down, each handed down — what is placed, the objects' record, a
    /// preview's cut its air; what the ways write; and the ground's, by its
    /// one answer for a node (pxl-portable `ground::node`): the Earth's past
    /// the service area, air over its tile's bound, else its column off the
    /// windows and the caves' value. The column and the ways' runs over it
    /// are read as the GPU reads them (pxl-portable `window::ground`,
    /// `window::ways_over`), off what it drew and read back.
    pub(super) fn drawn(&self, at: &mut At) -> Answer<Option<Block>> {
        let (zoom, b) = at.node;
        xpute_kit::ensure!(zoom >= coarsest(), EINVAL, zoom);
        let placed = placed::over(&self.objects.lanes(), at.placed, self.cut(self.carve), self.cut(self.hide), zoom, b.map(|v| v as i32), true);
        if placed.says == BLOCK {
            return Answer::Exact(Some(Block::Placed(placed.block)));
        }
        let (cave, mut shard, mut place) = (at.cave, at.shard, at.ground);
        let mut reading = Reading::new(self, at);
        let c = b.map(|v| v as i32);
        let (ground, cell, found) = ground::node(&mut reading, cave, &mut shard, &mut place, (zoom, c), [zoom, zoom], true);
        let answer = reading.answer(ground.kind != Kind::Lacks, || {
            let (g, _) = window::ways_over(&Windowed, shard, found, cell, zoom, c, ground.block);
            (g != u32::from(sentinel::void)).then_some(Block::Ground(g as GroundId))
        });
        (at.shard, at.ground) = (shard, place);
        answer
    }

    /// Whether the world holds nothing finer than `at`'s node, so every node
    /// under it is the node itself (the ground decision's "Level of detail is
    /// the world there"): the ground, its ways and the caves have no detail
    /// past the ground's cells, and nothing placed or carved is finer in it,
    /// none placed under it or one placed covering it whole.
    fn whole(&self, at: &At) -> bool {
        let (zoom, c) = at.node;
        zoom >= GROUND.cell() && placed::whole(at.placed, self.cut(self.carve), zoom, c.map(|v| v as i32))
    }

    /// A preview's entry as pxl-portable `placed` takes it.
    fn cut(&self, entry: Option<EntryAt>) -> Cut {
        entry.map_or(Cut::NONE, |e| Cut {
            zoom: e.zoom(),
            at: e.at().map(|v| v as i32),
        })
    }

    /// Its child `k`, in `child_of`'s order, with what it found handed down.
    pub(super) fn child(&self, at: &At, k: usize) -> At {
        let (zoom, c) = at.node;
        let (z, child) = (zoom + 1, child_of(c, k));
        At {
            node: (z, child),
            cave: at.cave.child(z, child.map(|v| v as i32)),
            placed: self.objects.child(&at.placed, (z, child), z),
            line: at.line,
            shard: at.shard,
            ground: at.ground,
        }
    }

    /// What is in `at`'s node, asked from `far` to `near`, the zooms a reader
    /// asks within it: at `near` (or finer), or where the world holds nothing
    /// finer than the node (`whole`), the block (`drawn`); coarser, what is
    /// placed under it splits it, else the ground's one answer for a node
    /// (pxl-portable `ground::node`), whether anything under it fills at
    /// those zooms.
    pub(super) fn node(&self, at: &mut At, [far, near]: [Zoom; 2]) -> Answer<In> {
        let (zoom, c) = at.node;
        if zoom >= near || self.whole(at) {
            return self.drawn(at).map(In::Block);
        }
        if placed::over(&self.objects.lanes(), at.placed, Cut::NONE, Cut::NONE, zoom, c.map(|v| v as i32), false).says == SPLIT {
            return Answer::Exact(In::Split);
        }
        let (cave, mut shard, mut place) = (at.cave, at.shard, at.ground);
        let mut reading = Reading::new(self, at);
        let (ground, _, _) = ground::node(&mut reading, cave, &mut shard, &mut place, (zoom, c.map(|v| v as i32)), [far, near], false);
        let answer = reading.answer(ground.kind != Kind::Lacks, || match ground.kind {
            Kind::Split => In::Split,
            _ => In::Empty,
        });
        (at.shard, at.ground) = (shard, place);
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
    fn shards(&self) -> u32 {
        source::Declared.shards()
    }
    fn shard(&self, i: u32) -> (Zoom, [i32; 2]) {
        source::Declared.shard(i)
    }
}

impl ground::Held for Reading<'_, '_> {
    /// The bound over the node of the tile `cell` reads its cells from, off
    /// its line (pxl-portable `Held::relief`); a node with no ground under
    /// it, `EMPTY`.
    fn relief(&mut self, place: &mut Found, cell: Zoom, _: (Zoom, [i32; 3])) -> ([i32; 2], bool) {
        let (zoom, c) = self.at.node;
        let line = self.world.line(self.at, cell);
        match self.take(self.world.terrain.relief(&line, place, (zoom, [c[0], c[1]]))) {
            Some(r) => (r.unwrap_or(ground::EMPTY), true),
            None => (ground::EMPTY, false),
        }
    }

    /// The block's column off the windows, the area's edge laid on it, as
    /// the GPU reads it (pxl-portable `window::ground`), stepped from where
    /// its relief found its cells.
    fn cell(&mut self, shard: Shard, place: Found, zoom: Zoom, c: [i32; 3]) -> (Cell, bool) {
        (window::ground(&Windowed, shard, place, zoom, [c[0], c[1]]).0, true)
    }
}
