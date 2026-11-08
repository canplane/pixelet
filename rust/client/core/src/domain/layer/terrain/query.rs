// pxl-core/domain/layer/terrain/query.rs

//! The terrain's answers to the query. A node's cells are read from the
//! clip levels (clipmap.rs), at the cells of the tile they fold from, using
//! the finest clip level that holds them. The answer is exact when that clip
//! level is at least as fine as the tile that the tile's line gives the
//! ground from, and coarser when only a coarser clip level holds them. A
//! column's ways are read from its brick's tree, whose child groups live in
//! the ways' pool (pool.rs).

use pxl_engine::area::Shard;
use pxl_engine::clipmap::{self as cells, Lookup};
use pxl_engine::ground::{Bounds, Cell};
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::hierarchy::tree::{ancestor_of, region, Node};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::quadrant::{Quadrant, TOP_ZOOM};

use crate::domain::layer::terrain::clipmap::CpuClipmap;
use crate::domain::layer::tile_cache::{Line, Lines, TileCache};
use crate::sys::io::source;

use crate::domain::layer::{Answer, Query};
use pxl_spec::space::coord::aabb::Aabb;

/// The terrain's reads beyond `Query`, the form it shares with the object
/// store: its relief, its columns, and the lines they are read on. As with
/// `Query`, only the world's query (domain/query.rs) reads it.
pub(in crate::domain) trait TerrainQuery {
    /// The lowest and highest heights, in leaf cells, of the ground over
    /// column `node`'s own cells, with the ways' heights kept apart
    /// (pxl-engine `ground::Bounds`). They are the bounds over the cells of
    /// `line`'s tile, taken from the finest clip level that holds them, in one
    /// look-up whatever the node's size. A coarser clip level standing in
    /// gives the bounds of its own cells, which contain the finer ones, and
    /// the answer names the tile lacked. Where no clip level holds the cells,
    /// the answer is that lack, never an empty bound.
    ///
    /// No clip level holds anything past the service area, so a reader asks
    /// that first, in the descent of its shard (pxl-engine `area::Shard`).
    /// `place` comes in as where a node over this one found its cells, or
    /// `Lookup::NONE`, and goes out as where this node's cells are
    /// (pxl-engine `clipmap::step`). The cells are read at the cell zooms in
    /// `zooms`, coarsest and finest, as the GPU reads them (pxl-engine
    /// `clipmap::bound_over`). `line` names the tile and says whether the
    /// clip level read gives the tile's own ground.
    fn relief(
        &self,
        line: &Line,
        place: &mut Lookup,
        node: Node<2>,
        zooms: [Zoom; 2],
    ) -> Answer<Bounds>;

    /// Column `node` as read from the clip levels, or the wall where the
    /// column is the wall, read as the GPU reads it (pxl-engine
    /// `clipmap::ground`) and stepped from `place`. The answer is exact where
    /// `line` gives the ground at the zoom of the clip level read or coarser,
    /// and otherwise coarser, naming the tile (`answer_at`). Where no clip
    /// level holds the column, the answer is the lack.
    fn column(&self, line: &Line, place: Lookup, node: Node<2>) -> Answer<Cell>;

    /// The line that `node`'s cells at `cell_zoom` are read on, stepped from
    /// where `lines` last walked. A tile under one that `lines` holds is
    /// stepped from that tile's line, and a coarser tile reuses the line an
    /// ancestor read. Nodes beside or under each other therefore share their
    /// lines down to where they part, and pay one look-up per zoom only past
    /// that point.
    fn walk_line(&self, lines: &mut Lines, node: Node<2>, cell_zoom: Zoom) -> Line;

    /// The line that `node`'s cells at `cell_zoom` are read on, found from
    /// `over`, the line its parent column's cells were read on. It is `over`
    /// itself where both are the same tile's, one step from `over` where
    /// `over` is the tile's parent's (`step`), and otherwise walked from the
    /// top as `seek` walks.
    fn line_under(&self, over: Option<&Line>, node: Node<2>, cell_zoom: Zoom) -> Line;

    /// `bound`, with the node's line found by `walk_line`. A walk over nodes
    /// in key order pays one look-up per node.
    fn bound_walked(
        &self,
        lines: &mut Lines,
        node: Node<2>,
        cell_zoom: Zoom,
    ) -> Answer<Option<Aabb>>;
}

impl TerrainQuery for TileCache<TerrainTile> {
    fn relief(
        &self,
        line: &Line,
        place: &mut Lookup,
        node: Node<2>,
        zooms: [Zoom; 2],
    ) -> Answer<Bounds> {
        let (r, from, here) = cells::bound_over(&CpuClipmap, place, zooms, node);
        answer_at(line, from, here, r)
    }

    fn column(&self, line: &Line, place: Lookup, (zoom, c): Node<2>) -> Answer<Cell> {
        let (cell, f) = cells::ground(&CpuClipmap, Shard::UNKNOWN, place, zoom, c);
        answer_at(line, f.from(), f.here(), cell)
    }

    fn walk_line(&self, lines: &mut Lines, node: Node<2>, cell_zoom: Zoom) -> Line {
        self.seek(lines, bound_tile(node, cell_zoom))
    }

    fn line_under(&self, over: Option<&Line>, node: Node<2>, cell_zoom: Zoom) -> Line {
        let tile = bound_tile(node, cell_zoom);
        match over {
            Some(o) if o.tile == tile => *o,
            Some(o) if tile.zoom > TOP_ZOOM && o.tile == tile.parent() => self.step(o, tile),
            _ => self.line(tile),
        }
    }

    fn bound_walked(
        &self,
        lines: &mut Lines,
        node: Node<2>,
        cell_zoom: Zoom,
    ) -> Answer<Option<Aabb>> {
        self.bound(&self.walk_line(lines, node, cell_zoom), node, cell_zoom)
    }
}

impl TileCache<TerrainTile> {
    /// `tile`'s line, stepped down from `from`, the line of `tile` itself or
    /// of a tile over it, at one look-up per zoom between them.
    fn down(&self, from: &Line, tile: Quadrant) -> Line {
        match tile == from.tile {
            true => *from,
            false => self.step(&self.down(from, tile.parent()), tile),
        }
    }
}

/// The tile a node's bound is read from: the tile its cells at `cell_zoom`
/// are read from, within the pyramid's zooms, or, where the node is wider
/// than that tile, the finest tile that holds the whole node.
fn bound_tile<const D: usize>((zoom, at): Node<D>, cell_zoom: Zoom) -> Quadrant {
    let read = GROUND.read_zoom(cell_zoom).min(zoom);
    Quadrant::new(read, ancestor_of([at[0], at[1]], zoom - read))
}

/// The answer for `v`, read from the clip level at zoom `from`; `here` is
/// false where no clip level held it. It is exact where `line` gives the
/// ground at `from` or coarser, and otherwise coarser, naming the line's
/// tile.
fn answer_at<T>(line: &Line, from: Zoom, here: bool, v: T) -> Answer<T> {
    match here {
        false => Answer::Nothing(line.tile),
        true if line.src.is_some_and(|s| from >= GROUND.cell_zoom(s.zoom)) => Answer::Exact(v),
        true => Answer::Coarser(v, line.tile),
    }
}

impl Query for TileCache<TerrainTile> {
    type Cursor = Line;

    fn cursor<const D: usize>(&self, node: Node<D>, cell_zoom: Zoom) -> Line {
        self.line(bound_tile(node, cell_zoom))
    }

    /// A child that reads its parent's tile or one under it steps down from
    /// the parent's line one zoom at a time (`down`). A child that reads a
    /// coarser tile, whose line the parent does not hold, walks from the top.
    fn child<const D: usize>(&self, parent: &Line, node: Node<D>, cell_zoom: Zoom) -> Line {
        let tile = bound_tile(node, cell_zoom);
        let mut over = tile;
        while over.zoom > parent.tile.zoom {
            over = over.parent();
        }
        match over == parent.tile {
            true => self.down(parent, tile),
            false => self.line(tile),
        }
    }

    /// The bound joins the clip levels' relief of the ground and of the ways
    /// over the node's own cells (`relief`), reaches one cell at `cell_zoom`
    /// below the lowest, and is cut to the node. A node past the service area
    /// has no palette index in it and holds nothing. For a node of a shard or
    /// finer, that is known before any of its ground arrives, the same way
    /// every reader knows it (pxl-engine `area::Shard`). A coarser node may
    /// straddle the area's edge, so it is read.
    fn bound<const D: usize>(
        &self,
        line: &Line,
        (zoom, at): Node<D>,
        cell_zoom: Zoom,
    ) -> Answer<Option<Aabb>> {
        let node = (zoom, [at[0], at[1]]);
        if Shard::UNKNOWN.at(&source::ServiceArea, node).past() {
            return Answer::Exact(None);
        }
        let whole = region((zoom, at));
        let mut place = Lookup::NONE;
        self.relief(line, &mut place, node, [cell_zoom, cell_zoom])
            .map(|[ground, ways]| {
                let [lo, hi] = [ground[0].min(ways[0]), ground[1].max(ways[1])];
                let lo = (f64::from(lo) * units(GROUND.leaf_cell_zoom()) - units(cell_zoom))
                    .max(whole.min[2]);
                let hi = (f64::from(hi) * units(GROUND.leaf_cell_zoom())).min(whole.max[2]);
                (lo <= hi).then(|| Aabb {
                    min: [whole.min[0], whole.min[1], lo],
                    max: [whole.max[0], whole.max[1], hi],
                })
            })
    }
}
