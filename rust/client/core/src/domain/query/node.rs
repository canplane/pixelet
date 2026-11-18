// pxl-core/domain/query/node.rs

//! What the layers compose in a node, the descent the world's one query
//! stands on (query.rs `World::node`, which adds the decor standing in it).
//! One record of a node answers for two sizes, the cell at the zoom asked and
//! a coarser node.
//!
//! At the zoom a reader asks, the answer is what the node is: the cell, by
//! the one rule (pxl-engine `ground::node`). Each source answers from its
//! own level of detail at that zoom and from what the node's parent found
//! and handed down (`At`; `World::asset` serves an asker with nothing handed
//! down):
//!
//! - the ground: a cell's off the clip levels, as the GPU reads it, or the
//!   wall at the service area's edge where the cell is that wall. The ways
//!   are read off the spans their brick's fill kept, which lie in the pool
//!   and are read as the GPU reads them (pxl-engine `clipmap::ground`,
//!   `clipmap::way_at`);
//! - the caves: the share of the cell they take (pxl-content `cave::hollow`);
//! - what is placed: the object pyramid record at that zoom, folded by
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
//!   elevations the clip levels hold under the node (`TileCache::relief`, off
//!   the finest clip level holding its cells). Every cell under the node at
//!   each zoom is asked against the ground;
//! - the ways: by their own rule (pxl-engine `way::slab`) over their own
//!   bound in the same levels. That bound is the leaf cells the ways write in
//!   each column (pxl-engine `way::lined_hull`), joined apart from the
//!   ground's (pxl-engine fill.rs `mip`). A way that a coarser tile drops as
//!   narrower than half its cell is dropped there too, as that level of
//!   detail does;
//! - the caves: nothing; a node a cave takes all of still splits, and its
//!   cells say so (pxl-engine `ground::node`);
//! - what is placed: whether the object pyramid has no record at the node.
//!
//! A source that does not hold the node answers that it lacks it, never air
//! and never that nothing fills.

use crate::domain::layer::Base as _;
use crate::domain::layer::terrain::tile::IndexedTerrainTile;
use pxl_content::Pixelet;
use pxl_engine::answer::{In, Kind};
use pxl_engine::clipmap::{self, Lookup};
use pxl_engine::ground::{self, Cell};
use pxl_engine::object::Cut;
use pxl_engine::world;
use pxl_spec::asset::cube::Sentinel;
use pxl_spec::space::coord::zoom::{Zoom, units};
use pxl_spec::space::hierarchy::tree::{Node, child_at};
use pxl_spec::space::layer::area::{Area, Shard};
use pxl_spec::space::layer::quadrant::Quadrant;

use super::change;
use super::detail::coarsest;
use super::world::World;
use crate::domain::layer::Answer;
use crate::domain::layer::terrain::clipmap::{self as terrain_clipmap, clip_read};
use crate::domain::layer::terrain::pool::way_pool;
use crate::domain::layer::tile_cache::Path;
use crate::sys::io::source;

/// A node as a descent reaches it, with X on the asker's lap, and what its
/// ancestors found, which it hands to its children (pxl-engine
/// `world::Inheritance`).
#[derive(Clone, Copy)]
pub(super) struct At {
    pub(super) node: Node<3>,
    pub(super) inheritance: world::Inheritance,
}

impl World {
    /// `node` with nothing handed down, as a descent's root is. Each source
    /// finds what it needs down the node's line from the top.
    pub(super) fn start(&self, node: Node<3>) -> At {
        At {
            node,
            inheritance: world::Inheritance::of(&self.object.lanes(), node),
        }
    }

    /// What is in the cell `node` at its own zoom, which is `coarsest` or
    /// finer, for an asker with nothing handed down: what `ask` answers fills
    /// it, asked at that zoom, or void where it is air or void. Each source
    /// finds what it needs down the node's line from the top. The cell's
    /// rect is reported as read (`change::report_read`).
    pub(super) fn asset_at(
        &self,
        node: Node<3>,
        ask: impl FnOnce(&mut At, [Zoom; 2], &mut Path) -> Answer<In<[u32; 2]>>,
    ) -> Answer<u32> {
        let (zoom, c) = node;
        xpute_kit::ensure!(zoom >= coarsest(), EINVAL, zoom);
        let side = units(zoom);
        let middle = [c[0], c[1]].map(|v| (f64::from(v) + 0.5) * side);
        change::report_read(middle, middle, zoom);
        ask(&mut self.start(node), [zoom, zoom], &mut Path::default()).map(|i| {
            if i.kind == Kind::Cell {
                i.asset[0]
            } else {
                Sentinel::Void as u32
            }
        })
    }

    /// `at`'s child `k`, in `child_of`'s order, with what `at` found handed
    /// down to it.
    pub(super) fn child(&self, at: &At, k: usize) -> At {
        let (zoom, c) = at.node;
        let node = (zoom + 1, child_at(zoom, c, k));
        At {
            node,
            inheritance: at.inheritance.child(&self.object.lanes(), node),
        }
    }

    /// What the layers compose in `at`'s node (`layers`), under the decor
    /// standing in it: what decor's own generation stands on and asks
    /// (domain/query/scatter.rs).
    pub(super) fn layer_node(
        &self,
        at: &mut At,
        range: [Zoom; 2],
        path: &mut Path,
    ) -> Answer<In<[u32; 2]>> {
        self.layers(at, range, path).0
    }

    /// What the layers answer of `at`'s node, asked over `range` (pxl-engine
    /// `world::node`), and the ground's bound they read over it, none where
    /// no bound was read. The ground is read as the GPU reads it, off the
    /// clip levels (`clip_read`), down the node's line through the tile cache
    /// (`GroundRead`).
    ///
    /// The answer (pxl-engine `answer`) is one of three: a split, with its
    /// box in its own steps; the asset of a cell at the zoom asked, void for
    /// air; or, for a node coarser than asked with nothing under it filling,
    /// air of the node's size, which its zoom against `range` tells apart.
    /// The four asset ids a cell is drawn by are left to the GPU.
    /// `path` are the ground's lines the asking walk read, and the node's
    /// line steps on from them (`walk_line`).
    pub(super) fn layers(
        &self,
        at: &mut At,
        range: [Zoom; 2],
        path: &mut Path,
    ) -> (Answer<In<[u32; 2]>>, Option<ground::Extent>) {
        let lanes = self.object.lanes();
        let mut reading = GroundRead::new(path);
        let i = world::node(
            &mut reading,
            &clip_read(),
            &lanes,
            way_pool(),
            &mut at.inheritance,
            at.node,
            range,
            [Cut::NONE; 2],
        );
        (reading.answer(i.kind != Kind::Lack, || i), reading.relief)
    }
}

/// The ground as a node's reading holds it (pxl-engine `ground::Resident`):
/// the world's tile cache down the node's line, the lines its walk read, and
/// the first tile the reading lacked (`lack`) or held only through
/// a coarser tile standing in for it (`coarser`), and the bound it read over
/// the node (`relief`).
struct GroundRead<'r> {
    path: &'r mut Path,
    lack: Option<Quadrant>,
    coarser: Option<Quadrant>,
    relief: Option<ground::Extent>,
}

impl<'r> GroundRead<'r> {
    fn new(path: &'r mut Path) -> GroundRead<'r> {
        GroundRead {
            path,
            lack: None,
            coarser: None,
            relief: None,
        }
    }

    /// `of` as the reading went. Where `found`, it is exact, or coarser with
    /// the first tile that only a coarser one stood in for. Otherwise the
    /// answer is a lack of the tile the reading lacked.
    fn answer<T>(&self, found: bool, of: impl FnOnce() -> T) -> Answer<T> {
        match (found, self.lack, self.coarser) {
            (false, Some(l), _) => Answer::Lack(l),
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
            Answer::Lack(l) => {
                self.lack = Some(l);
                None
            }
        }
    }
}

impl Area for GroundRead<'_> {
    fn ends(&self) -> bool {
        source::ServiceArea.ends()
    }
    fn nine(&self, key: u32) -> u32 {
        source::ServiceArea.nine(key)
    }
}

impl ground::Resident for GroundRead<'_> {
    type Content = Pixelet;
    type Place = Lookup;
    const NOWHERE: Lookup = Lookup::NONE;
    /// The bound over the node of its cells at the zooms `cell_range`, off the
    /// clip levels, stepped from `place` (pxl-engine
    /// `ground::Resident::relief`). The node's line names the tile that a
    /// lack or a coarser clip level stands for. Where the bound is lacking,
    /// it is `EMPTY` with false. Where a coarser clip level's cells stand in
    /// for a brick under the node (pxl-engine `clipmap::stood_in`), a ray's
    /// answer says what it read, so nothing bounds the node
    /// (`ground::UNBOUNDED`): it splits, and the nodes under it answer each
    /// for what it holds.
    fn relief(
        &mut self,
        place: &mut Lookup,
        cell_range: [Zoom; 2],
        (zoom, c): (Zoom, [i32; 3]),
    ) -> (ground::Extent, bool) {
        let line = IndexedTerrainTile::tile_cache().walk_line(
            self.path,
            (zoom, [c[0], c[1]]),
            cell_range[1],
        );
        let node = (zoom, [c[0], c[1]]);
        self.relief = match IndexedTerrainTile::tile_cache().relief(&line, place, node, cell_range)
        {
            Answer::Coarser(..) if terrain_clipmap::stood_in(cell_range, node) => {
                Some([ground::UNBOUNDED; 2])
            }
            answer => self.take(answer),
        };
        (
            self.relief.unwrap_or([ground::EMPTY; 2]),
            self.relief.is_some(),
        )
    }

    /// The cell's ground off the clip levels as the GPU reads it (pxl-engine
    /// `clipmap::ground`), or the wall where the cell is the wall. The
    /// lookup steps on from where the node's relief found its cells.
    fn cell(&mut self, shard: Shard, place: Lookup, zoom: Zoom, c: [i32; 3]) -> Cell {
        clipmap::ground(&clip_read(), shard, place, zoom, [c[0], c[1]]).0
    }
}
