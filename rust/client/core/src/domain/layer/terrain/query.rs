// pxl-core/domain/layer/terrain/query.rs

//! The terrain's answers to the query. A node's cells are read from the
//! clip levels (clipmap.rs), at the cells of the tile they fold from, using
//! the finest clip level that holds them. The answer is exact when that clip
//! level is at least as fine as the tile that the tile's line gives the
//! ground from, and coarser when only a coarser clip level holds them. A
//! column's ways are read from the spans its brick's fill kept, a run in the
//! ways' pool (pool.rs; pxl-engine `clipmap::way_at`).

use pxl_engine::clipmap::{self as cells, Lookup};
use pxl_engine::ground::{Cell, Extent};
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::hierarchy::tree::{ancestor_of, region, Node};
use pxl_spec::space::layer::area::Shard;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::quadrant::Quadrant;

use crate::domain::layer::terrain::clipmap::{self, clip_read};
use crate::domain::layer::tile_cache::{Line, Path, TileCache};
use crate::sys::io::source;

use crate::domain::layer::Answer;
use pxl_spec::space::coord::aabb::Aabb;

/// What only the world's query (domain/query.rs) reads of the ground: a node's
/// bound and the cursor a descent carries to it, the ground's relief and
/// columns, and the lines they are read on. A node names part of the hierarchy
/// the ground shares with the objects; it says what it asks and how fine its
/// cells are, and the answer says what it lacked (`Answer`).
impl TileCache<TerrainTile> {
    /// The cursor of `node` asked alone, the line its cells at `cell_zoom` are
    /// read on. It equals what a descent from the top would carry to `node`.
    pub(in crate::domain::layer) fn cursor(&self, node: Node<2>, cell_zoom: Zoom) -> Line {
        self.line(bound_tile(node, cell_zoom))
    }

    /// A box around everything that may show in `node` when its cells are
    /// read at `cell_zoom`, computed from the node's cursor, or none where
    /// nothing shows. The box covers what is stored down to the depth where
    /// everything around it is solid, since nothing past that depth is seen.
    /// What is not known is carried by the `Answer`, not by the box, so a
    /// bound has no third state. The node stands for the whole column over
    /// it.
    ///
    /// The bound joins the clip levels' relief of the ground and of the ways
    /// over the node's own cells (`relief`), reaches one cell at `cell_zoom`
    /// below the lowest, and is cut to the node. A node past the service area
    /// has no palette index in it and holds nothing. For a node of a shard or
    /// finer, that is known before any of its ground arrives, the same way
    /// every reader knows it (pxl-spec `area::Shard`). A coarser node may
    /// straddle the area's edge, so it is read.
    pub(in crate::domain::layer) fn bound(
        &self,
        line: &Line,
        node: Node<2>,
        cell_zoom: Zoom,
    ) -> Answer<Option<Aabb>> {
        if Shard::UNKNOWN.at(&source::ServiceArea, node).past() {
            return Answer::Exact(None);
        }
        let whole = region(node);
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

    /// `bound`, with the node's line found by `walk_line`. A walk over nodes
    /// in key order pays one look-up per node.
    pub(in crate::domain::layer) fn bound_walked(
        &self,
        path: &mut Path,
        node: Node<2>,
        cell_zoom: Zoom,
    ) -> Answer<Option<Aabb>> {
        self.bound(&self.walk_line(path, node, cell_zoom), node, cell_zoom)
    }

    /// The lowest and highest heights, in leaf cells, of the ground over
    /// column `node`'s own cells, with the ways' heights kept apart
    /// (pxl-engine `ground::Extent`). They are the bounds over the cells of
    /// `line`'s tile, taken from the finest clip level of each center that
    /// holds them (clipmap.rs `bound_over`), whatever the node's size. A
    /// coarser clip level standing in gives the bounds of its own cells, and
    /// the answer names the tile lacked. Where no clip level holds the cells,
    /// the answer is that lack, never an empty bound.
    ///
    /// No clip level holds anything past the service area, so a reader asks
    /// that first, in the descent of its shard (pxl-spec `area::Shard`).
    /// `place` comes in as where a node over this one found its cells, or
    /// `Lookup::NONE`, and goes out as where this node's cells are
    /// (pxl-engine `clipmap::step`). The cells are read at the cell zooms in
    /// `zooms`, coarsest and finest, as the GPU reads them (pxl-engine
    /// `clipmap::bound_over`). `line` names the tile and says whether the
    /// clip level read gives the tile's own ground.
    pub(in crate::domain::layer) fn relief(
        &self,
        line: &Line,
        place: &mut Lookup,
        node: Node<2>,
        zooms: [Zoom; 2],
    ) -> Answer<Extent> {
        let (r, from, here) = clipmap::bound_over(place, zooms, node);
        answer_at(line, from, here, r)
    }

    /// Column `node` as read from the clip levels, or the wall where the
    /// column is the wall, read as the GPU reads it (pxl-engine
    /// `clipmap::ground`) and stepped from `place`. The answer is exact where
    /// `line` gives the ground at the zoom of the clip level read or coarser,
    /// and otherwise coarser, naming the tile (`answer_at`). Where no clip
    /// level holds the column, the answer is the lack.
    pub(in crate::domain::layer) fn column(
        &self,
        line: &Line,
        place: Lookup,
        (zoom, c): Node<2>,
    ) -> Answer<Cell> {
        let (cell, f) = cells::ground(&clip_read(), Shard::UNKNOWN, place, zoom, c);
        answer_at(line, f.from(), f.here(), cell)
    }

    /// The line that `node`'s cells at `cell_zoom` are read on, stepped from
    /// where `path` last walked. A tile under one that `path` holds is
    /// stepped from that tile's line, and a coarser tile reuses the line an
    /// ancestor read. Nodes beside or under each other therefore share their
    /// lines down to where they part, and pay one look-up per zoom only past
    /// that point.
    pub(in crate::domain::layer) fn walk_line(
        &self,
        path: &mut Path,
        node: Node<2>,
        cell_zoom: Zoom,
    ) -> Line {
        self.seek(path, bound_tile(node, cell_zoom))
    }
}

/// The tile a node's bound is read from: the tile its cells at `cell_zoom`
/// are read from, within the pyramid's zooms, or, where the node is wider
/// than that tile, the finest tile that holds the whole node.
fn bound_tile((zoom, at): Node<2>, cell_zoom: Zoom) -> Quadrant {
    let read = GROUND.read_zoom(cell_zoom).min(zoom);
    Quadrant::new(read, ancestor_of(at, zoom - read))
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
