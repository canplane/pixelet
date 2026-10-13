// pxl-kernel/query/node.rs

//! What is in a node: the world's one query
//! (decision/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md,
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
//! - the ground: by its one rule (`sample::lay`, as `carves` asks it of a
//!   block) over the lowest and highest its tile index holds under the node
//!   (`Residency::relief`, its lane the union of the finer ones'), every cell
//!   under it at each zoom asked over the ground, or ground a cave may take
//!   with a cave through all of it;
//! - the ways: the ground's index pyramid as it is, which carries the ways'
//!   heights with the ground's; a way a coarser tile drops as narrower than
//!   half its cell is dropped there, as its level of detail does;
//! - the caves: the hash node hollow and drawing nothing again under it
//!   (`procedural::Cave`, one draw of its parent's);
//! - what is placed: no record of the objects' pyramid at the node.
//!
//! A source not holding the node answers that it lacks it, never air and
//! never that nothing fills.

use pxl_spec::block::sample::{at as one_rule, lay, walled, Grid, Lay};
use pxl_spec::block::Block;
use pxl_spec::procedural::Cave;
use pxl_spec::space::coord::world::wrap_cell;
use pxl_spec::space::coord::zoom::{cube, units, Zoom, SHARD_ZOOM};
use pxl_spec::space::hierarchy::tree::{child_of, Node};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;
use xpute_guest::global::Global;

use super::block::earth;
use super::world::World;
use crate::io::source;
use crate::layer::object::pyramid::Held;
use crate::layer::object::store::AIR;
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

    /// A node with nothing handed down, a descent's root among them: each
    /// source found down its line from the top.
    pub(super) fn start(&self, (zoom, at): Node<3>) -> At {
        At {
            node: (zoom, at),
            cave: Cave::of(zoom, at),
            placed: self.objects.cursor((zoom, wrap_cell(zoom, at)), zoom),
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
    /// where it is air or void, else what fills it or not, by the one rule
    /// over each source's answer there, each handed down — what is placed,
    /// the objects' record, a preview's cut its air; the ground's cell off
    /// the tile drawing that zoom's cells, with its crossings and the ring
    /// about it; and the caves' value.
    fn drawn(&self, at: &mut At) -> Answer<Option<Block>> {
        let (zoom, b) = at.node;
        xpute_kit::ensure!(zoom >= coarsest(), EINVAL, zoom);
        // Past the service area: the Earth's ground, solid to sea level. The
        // area runs on shards' edges, which no block straddles.
        if source::past_at([0, 1].map(|k| (b[k] as f64 + 0.5) * units(zoom))) {
            return Answer::Exact(earth(zoom, b));
        }
        let placed = match self.carve.is_some_and(|e| cube(e.zoom(), e.at()).shares(cube(zoom, b))) {
            true => Some(AIR),
            false => self.objects.value(at.placed, zoom, wrap_cell(zoom, b), self.hide),
        };
        let line = self.line(at, zoom);
        self.ground(&line, (zoom, [b[0], b[1]]), zoom, 1).and_then(|g| {
            let grid = CELL.get(Grid::default);
            match grid.read(&g.around(), zoom, g.tile.zoom, g.first_at(zoom), [b[0] - 1, b[1] - 1], 3) {
                Ok(()) => Answer::Exact(one_rule(grid, b, placed, at.cave.hollow())),
                // The heap refused a 3 × 3 grid's room.
                Err(_) => Answer::Nothing(g.tile),
            }
        })
    }

    /// Whether the world holds nothing finer than `at`'s node, so every node
    /// under it is the node itself (the ground decision's "Level of detail is
    /// the world there"): the ground, its ways and the caves have no detail
    /// past the ground's cells, and nothing placed or carved is finer in it,
    /// none placed under it or one placed covering it whole.
    fn whole(&self, at: &At) -> bool {
        let (zoom, c) = at.node;
        let carved = self.carve.is_some_and(|e| e.zoom() > zoom && cube(e.zoom(), e.at()).shares(cube(zoom, c)));
        zoom >= GROUND.cell() && matches!(at.placed, Held::Nothing | Held::Leaf { .. }) && !carved
    }

    /// Its child `k`, in `child_of`'s order, with what it found handed down.
    pub(super) fn child(&self, at: &At, k: usize) -> At {
        let (zoom, c) = at.node;
        let (z, child) = (zoom + 1, child_of(c, k));
        At {
            node: (z, child),
            cave: at.cave.child(z, child),
            placed: self.objects.child(&at.placed, (z, wrap_cell(z, child)), z),
            line: at.line,
        }
    }

    /// What is in `at`'s node, asked from `far` to `near`, the zooms a reader
    /// asks within it: at `near` (or finer), or where the world holds nothing
    /// finer than the node (`whole`), the block (`drawn`); coarser, whether
    /// anything under it fills at those zooms.
    pub(super) fn node(&self, at: &mut At, [far, near]: [Zoom; 2]) -> Answer<In> {
        let (zoom, c) = at.node;
        if zoom >= near || self.whole(at) {
            return self.drawn(at).map(In::Block);
        }
        let whole = cube(zoom, c);
        if at.placed != Held::Nothing {
            return Answer::Exact(In::Split);
        }
        // Past the service area: the Earth's ground, solid to sea level. The
        // area runs on shards' edges, which no node a shard or finer straddles.
        if zoom >= SHARD_ZOOM && source::past_at([0, 1].map(|k| (whole.min[k] + whole.max[k]) / 2.0)) {
            return Answer::Exact(if whole.min[2] >= 0.0 { In::Empty } else { In::Split });
        }
        // Coarser than any tile, no one index holds it.
        if zoom < GROUND.top() {
            return Answer::Exact(In::Split);
        }
        // The ground's line at the cells a tile at the node's zoom reads, no
        // finer than asked. Not held, no block under it is: the lack, never
        // that nothing fills.
        let line = self.line(at, near.min(GROUND.grid_zoom(zoom.min(LEAF_ZOOM))));
        self.terrain.relief(&line, (zoom, [c[0], c[1]])).map(|relief| {
            let Some([lo, hi]) = relief else { return In::Empty };
            // Coarser than a shard, the Earth past the service area may be in
            // it too, its face at sea level.
            let [lo, hi] = (if zoom < SHARD_ZOOM { [lo.min(0), hi.max(0)] } else { [lo, hi] }).map(f64::from);
            // At each zoom asked, every cell under it over the ground, or
            // ground a cave may take with a cave through all of it, by the
            // ground's one rule.
            let hollow = at.cave.whole(zoom) == Some(true);
            let empty = (far.max(zoom + 1)..=near).all(|z| {
                let u = units(z);
                let blocks = [(whole.min[2] / u).floor() as i64, (whole.max[2] / u).ceil() as i64];
                let wall = walled(z, source::past_at, [whole.min[0], whole.min[1]], [whole.max[0], whole.max[1]]);
                match lay(z, [lo, hi], wall, blocks) {
                    Lay::Air => true,
                    Lay::Open => hollow,
                    Lay::Kept => false,
                }
            });
            if empty {
                In::Empty
            } else {
                In::Split
            }
        })
    }
}
