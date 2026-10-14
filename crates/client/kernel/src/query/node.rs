// pxl-kernel/query/node.rs

//! What is in a node: the world's one query
//! (doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md,
//! "The query between them"), one record of a node answering two sizes.
//!
//! At the zoom a reader asks, it is what the node is, the block, by the one
//! rule (`pxl-spec block/sample.rs` `at`), each source answering from its own
//! level of detail at that zoom and from what the node's parent found,
//! handed down (`At`; `World::block` for an asker with none):
//!
//! - the ground: its tile drawing that zoom's cells, the cell's height and
//!   face as it folds them, and the ways whose boxes reach the cell, found
//!   on that tile (`Grid::read`, the cell and the ring about it);
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
//! - the ground: by its one rule (pxl-portable `ground::lay`, as a cell's
//!   block asks it of one block) over the lowest and highest its tile index
//!   holds under the node (`Residency::relief`, its lane the union of the
//!   finer ones'), every cell under it at each zoom asked over the ground, or
//!   ground a cave may take with a cave through all of it;
//! - the ways: the ground's index pyramid as it is, which carries the ways'
//!   heights with the ground's; a way a coarser tile drops as narrower than
//!   half its cell is dropped there, as its level of detail does;
//! - the caves: the hash node hollow and drawing nothing again under it
//!   (`procedural::Cave`, one draw of its parent's);
//! - what is placed: no record of the objects' pyramid at the node.
//!
//! A source not holding the node answers that it lacks it, never air and
//! never that nothing fills.

use pxl_portable::area::Area;
use pxl_portable::cast::Kind;
use pxl_portable::cube::sentinel;
use pxl_portable::ground::{self, Cell};
use pxl_portable::mask::{filled, mask};
use pxl_spec::bake::constant::EntryAt;
use pxl_spec::bake::tile::terrain::GroundId;
use pxl_spec::block::placed::{self, Cut, Held, BLOCK, SPLIT};
use pxl_spec::block::sample::Grid;
use pxl_spec::block::Block;
use pxl_spec::procedural::Cave;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::hierarchy::tree::{child_of, Node};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::Tile;
use xpute_guest::global::Global;

use super::world::World;
use crate::io::source;
use crate::layer::residency::Line;
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
}

/// The grid a block reads its cell and the ring about it into, kept so a
/// settled ask takes nothing from the heap.
static CELL: Global<Grid> = Global::new();

impl World<'_> {
    /// What is in the block `b` of `zoom`, X on the asker's lap, at its own
    /// zoom, `coarsest` or finer, for an asker with nothing handed down
    /// (`drawn`).
    pub fn block(&self, node: Node<3>) -> Answer<Option<Block>> {
        self.drawn(&mut self.start(node))
    }

    /// The face bits of the block `b` of `zoom` (pxl-portable `mask`): the
    /// blocks about it asked of the world's one query (`block`), each once,
    /// as a chunk's box reads its own; one not held fills nothing.
    pub fn faces(&self, (zoom, b): Node<3>) -> u32 {
        mask(filled(|[x, y, z]| {
            let n = [b[0] + i64::from(x), b[1] + i64::from(y), b[2] + i64::from(z)];
            self.block((zoom, n)).value().flatten().is_some_and(Block::fills)
        }))
    }

    /// A node with nothing handed down, a descent's root among them: each
    /// source found down its line from the top.
    pub(super) fn start(&self, (zoom, at): Node<3>) -> At {
        At {
            node: (zoom, at),
            cave: Cave::of(zoom, at.map(|v| v as i32)),
            placed: self.objects.cursor((zoom, at), zoom),
            line: None,
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
    /// the service area, air over its tile's bound, else its cell off the
    /// tile drawing that zoom's cells, with the ring about it, and the caves'
    /// value.
    fn drawn(&self, at: &mut At) -> Answer<Option<Block>> {
        let (zoom, b) = at.node;
        xpute_kit::ensure!(zoom >= coarsest(), EINVAL, zoom);
        let placed = placed::over(&self.objects.lanes(), at.placed, self.cut(self.carve), self.cut(self.hide), zoom, b.map(|v| v as i32), true);
        if placed.says == BLOCK {
            return Answer::Exact(Some(Block::Placed(placed.block)));
        }
        let cave = at.cave;
        let mut reading = Reading::new(self, at);
        let ground = ground::node(&mut reading, cave, (zoom, b.map(|v| v as i32)), [zoom, zoom], true);
        // The ways are read with the cell, and stand within its tile's bound.
        let grid = reading.read.then(|| CELL.get(Grid::default));
        reading.answer(ground.kind != Kind::Lacks, || match grid {
            Some(grid) => grid.ways_over(b, ground.block),
            None => (ground.block.0 != u32::from(sentinel::void)).then(|| {
                let g = ground.block.0 as GroundId;
                Block::Ground(g, ground.block.1.map(|v| v as GroundId))
            }),
        })
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
        let cave = at.cave;
        let mut reading = Reading::new(self, at);
        let ground = ground::node(&mut reading, cave, (zoom, c.map(|v| v as i32)), [far, near], false);
        reading.answer(ground.kind != Kind::Lacks, || match ground.kind {
            Kind::Split => In::Split,
            _ => In::Empty,
        })
    }
}

/// The ground as a node's reading holds it (pxl-portable `ground::Held`):
/// the world's residency down the node's line, the first tile it found
/// lacked or only a coarser one standing in for, and whether the cell's grid
/// was read, so the ways over it can be.
struct Reading<'r, 'a> {
    world: &'r World<'a>,
    at: &'r mut At,
    lacked: Option<Tile>,
    coarser: Option<Tile>,
    read: bool,
}

impl<'r, 'a> Reading<'r, 'a> {
    fn new(world: &'r World<'a>, at: &'r mut At) -> Reading<'r, 'a> {
        Reading {
            world,
            at,
            lacked: None,
            coarser: None,
            read: false,
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
    /// The bound over the node of the tile the finest asked zoom reads its
    /// cells from, off its line, as the GPU's (gpu-query-shader `relief`);
    /// none (`UNBOUNDED`) for a node wider than that tile; a node with no
    /// ground under it, lowest over highest.
    fn relief(&mut self, finest: Zoom, _: (Zoom, [i32; 3])) -> ([i32; 2], bool) {
        let (zoom, c) = self.at.node;
        if GROUND.read_zoom(finest) > zoom {
            return (ground::UNBOUNDED, true);
        }
        let line = self.world.line(self.at, finest);
        let none = [i32::from(i16::MAX), i32::from(i16::MIN)];
        match self.take(self.world.terrain.relief(&line, (zoom, [c[0], c[1]]))) {
            Some(r) => (r.unwrap_or(none), true),
            None => (none, false),
        }
    }

    /// The block's cell and the ring about it read into the grid (`CELL`),
    /// as the ways over it are read with it.
    fn cell(&mut self, zoom: Zoom, _: [i32; 3]) -> (Cell, bool) {
        let (_, b) = self.at.node;
        let none = Cell {
            look: u32::from(sentinel::void),
            top: 0,
            fold: [u32::from(sentinel::void); 4],
        };
        let line = self.world.line(self.at, zoom);
        let Some(g) = self.take(self.world.ground(&line, (zoom, [b[0], b[1]]), zoom, 1)) else {
            return (none, false);
        };
        let grid = CELL.get(Grid::default);
        match grid.read(&g.around(), zoom, g.tile.zoom, g.first_at(zoom), [b[0] - 1, b[1] - 1], 3) {
            Ok(()) => {
                self.read = true;
                (grid.cell_at([b[0], b[1]]).unwrap_or(none), true)
            }
            // The heap refused a 3 × 3 grid's room.
            Err(_) => {
                self.lacked = Some(g.tile);
                (none, false)
            }
        }
    }
}
