// pxl-kernel/domain/layer/terrain/query.rs

//! Terrain's answers. A node's cells are read from the windows
//! (domain/layer/terrain/window.rs) at the cells of the tile its cells fold from,
//! off the finest window holding them: exact where that is the zoom its
//! tile's line says its ground at, coarser where only one over holds them.
//! A column's ways are its square's tree the GPU built, its families read
//! back into the pool's mirror (domain/layer/terrain/window.rs `pool`).

use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::hierarchy::tree::{ancestor_of, region, Node};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{tile_at, Tile, TOP_ZOOM};
use pxl_spec_core::area::Shard;
use pxl_spec_core::ground::{Bounds, Cell};
use pxl_spec_core::window::{self as cells, Found};

use crate::domain::layer::residency::{Line, Lines, Residency};
use crate::domain::layer::terrain::window::Windowed;
use crate::sys::io::source;

use crate::domain::layer::{Answer, Query};
use pxl_spec::space::coord::aabb::Aabb;

impl Residency<TerrainTile> {
    /// The lowest and highest the ground holds over the column `node`'s own
    /// cells, in leaf cells, and the ways apart (pxl-spec-core
    /// `ground::Bounds`): the bounds over the cells of `line`'s tile, off
    /// the finest window holding them, one look-up whatever the node's size
    /// (`relief`). A coarser window standing in holds its own cells, which
    /// its bound holds, and names the tile lacked; where no window holds it,
    /// the lack, never nothing. Past the service area no window holds
    /// anything: a reader asks that first, the descent of its shard
    /// (pxl-spec-core `area::Shard`). `place` is where a node over it found
    /// its cells, or `Found::NONE`, and becomes where its own are
    /// (pxl-spec-core `window::step`). Its cells are read at `cell` as the
    /// GPU reads them (pxl-spec-core `window::bound_over`); `line` names the
    /// tile and says whether the window read is its ground's own.
    pub fn relief(
        &self,
        line: &Line,
        place: &mut Found,
        node: Node<2>,
        zooms: [Zoom; 2],
    ) -> Answer<Bounds> {
        let (r, from, here) = cells::bound_over(&Windowed, place, zooms, node);
        held(line, from, here, r)
    }

    /// The column `c` at `zoom` off the windows, the area's edge laid on
    /// it, as the GPU reads it (pxl-spec-core `window::ground`), stepped from
    /// `place`: exact where `line`
    /// says the ground at the window it was read off or coarser, else
    /// coarser, naming its tile (`held`); where no window holds it, the
    /// lack.
    pub fn column(&self, line: &Line, place: Found, (zoom, c): Node<2>) -> Answer<Cell> {
        let (cell, f) = cells::ground(&Windowed, place, zoom, c);
        held(line, f.from(), f.here(), cell)
    }

    /// The line `node`'s cells at `cell` are read on, stepped from where
    /// `lines` last walked: a tile under one it holds from that one's line, a
    /// coarser one its own as an ancestor read it, so nodes beside or under
    /// each other share their lines down to where they part, and pay a
    /// look-up a zoom only past it.
    pub fn line_walked(&self, lines: &mut Lines, node: Node<2>, cell: Zoom) -> Line {
        self.seek(lines, bound_tile(node, cell))
    }

    /// The line `node`'s cells at `cell` are read on, off `over`, the line its
    /// parent column's were: that line where it is the same tile's, stepped
    /// from it where it is the tile's parent's (`step`), else walked from the
    /// top, as `seek` steps.
    pub fn line_under(&self, over: Option<&Line>, node: Node<2>, cell: Zoom) -> Line {
        let tile = bound_tile(node, cell);
        match over {
            Some(o) if o.tile == tile => *o,
            Some(o) if tile.zoom > TOP_ZOOM && o.tile == tile.parent() => self.step(o, tile),
            _ => self.line(tile),
        }
    }

    /// `relief` over `node`'s cells at `cell`, its line walked (`line_walked`).
    pub fn relief_walked(
        &self,
        lines: &mut Lines,
        place: &mut Found,
        node: Node<2>,
        cell: Zoom,
    ) -> Answer<Bounds> {
        self.relief(
            &self.line_walked(lines, node, cell),
            place,
            node,
            [cell, cell],
        )
    }

    /// `bound`, its line walked (`line_walked`): a walk in key order pays one
    /// look-up a node.
    pub fn bound_walked(
        &self,
        lines: &mut Lines,
        node: Node<2>,
        cell: Zoom,
    ) -> Answer<Option<Aabb>> {
        self.bound(&self.line_walked(lines, node, cell), node, cell)
    }
}

impl Residency<TerrainTile> {
    /// `tile`'s line, stepped down from `from`'s, a tile over it or itself:
    /// a look-up a zoom between them.
    fn down(&self, from: &Line, tile: Tile) -> Line {
        match tile == from.tile {
            true => *from,
            false => self.step(&self.down(from, tile.parent()), tile),
        }
    }
}

/// The tile a node's bound is read from: its cells' tile, within the
/// pyramid's zooms, or where the node is wider than one, its own, the
/// finest one tile holding it.
fn bound_tile<const D: usize>((zoom, at): Node<D>, cell: Zoom) -> Tile {
    let read = GROUND.read_zoom(cell).min(zoom);
    tile_at(read, ancestor_of([at[0], at[1]], zoom - read))
}

/// `v`, read off the window at `from` (`here`, else none held): exact where
/// `line` says the ground at `from` or coarser, else coarser, naming its tile.
fn held<T>(line: &Line, from: Zoom, here: bool, v: T) -> Answer<T> {
    match here {
        false => Answer::Nothing(line.tile),
        true if line
            .source
            .is_some_and(|s| from >= GROUND.cell_zoom(s.zoom)) =>
        {
            Answer::Exact(v)
        }
        true => Answer::Coarser(v, line.tile),
    }
}

impl Query for Residency<TerrainTile> {
    type Cursor = Line;

    fn cursor<const D: usize>(&self, node: Node<D>, cell: Zoom) -> Line {
        self.line(bound_tile(node, cell))
    }

    /// The tile a child reads is its parent's or one under it, stepped down
    /// from the parent's line a zoom at a time (`down`); a coarser one, whose
    /// line no child holds, is walked from the top.
    fn child<const D: usize>(&self, parent: &Line, node: Node<D>, cell: Zoom) -> Line {
        let tile = bound_tile(node, cell);
        let mut over = tile;
        while over.zoom > parent.tile.zoom {
            over = over.parent();
        }
        match over == parent.tile {
            true => self.down(parent, tile),
            false => self.line(tile),
        }
    }

    /// From the windows' relief over the node's own cells (`relief`), the
    /// ground's and the ways' joined, down to a cell at `cell` under its
    /// lowest, within the node. A node past the service area, no look in
    /// it, holds nothing: known before any of its ground arrives, a shard or
    /// finer, as every reader knows it (pxl-spec-core `area::Shard`); a
    /// coarser node may straddle the area's edge and is read.
    fn bound<const D: usize>(
        &self,
        line: &Line,
        (zoom, at): Node<D>,
        cell: Zoom,
    ) -> Answer<Option<Aabb>> {
        let node = (zoom, [at[0], at[1]]);
        if Shard::UNKNOWN.at(&source::Declared, node).past() {
            return Answer::Exact(None);
        }
        let whole = region((zoom, at));
        let mut place = Found::NONE;
        self.relief(line, &mut place, node, [cell, cell])
            .map(|[ground, ways]| {
                let [lo, hi] = [ground[0].min(ways[0]), ground[1].max(ways[1])];
                let lo = (f64::from(lo) * units(GROUND.cell()) - units(cell)).max(whole.min[2]);
                let hi = (f64::from(hi) * units(GROUND.cell())).min(whole.max[2]);
                (lo <= hi).then(|| Aabb {
                    min: [whole.min[0], whole.min[1], lo],
                    max: [whole.max[0], whole.max[1], hi],
                })
            })
    }
}
