// pxl-kernel/layer/terrain/query.rs

//! Terrain's answers. A node's cells are read from the windows
//! (layer/terrain/window.rs) at the cells of the tile its cells fold from,
//! off the finest window holding them: exact where that is the zoom its
//! tile's line says its ground at, coarser where only one over holds them.
//! Its lines are read from the resident tile it is drawn from.

use pxl_portable::window::{self as cells, Squares as _, SQUARE_SHIFT};
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::block::cube::Ground;
use pxl_spec::block::limit::bedrock;
use pxl_spec::block::sample::TileSource;
use pxl_spec::procedural::Frame;
use pxl_spec::space::coord::q3;
use pxl_spec::space::coord::world::point;
use pxl_spec::space::coord::zoom::{base_of, count, units, Zoom, SHARD_ZOOM};
use pxl_spec::space::hierarchy::tree::{ancestor_of, region, Node};
use pxl_spec::space::layer::lattice::Cells;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{tile_at, Tile};

use crate::io::source;
use crate::layer::residency::{Line, Residency};
use crate::layer::terrain::window::Windowed;
use pxl_portable::window::FIRST_CELL;

use crate::layer::{Answer, Bound, Query, Relief};

/// Where a node's cells are read from: the tile that answered, and where the
/// first cell of the tile asked lies in it, `n` of the asked tile's cells to
/// one of its own.
#[derive(Clone, Copy)]
pub struct Source<'a> {
    pub tile: &'a TerrainTile,
    pub from: Tile,
    pub origin: [i64; 2],
    pub n: u32,
}

impl<'a> Source<'a> {
    pub fn view(&self) -> TileSource<'a> {
        TileSource {
            separation: &self.tile.separation,
            frame: Frame::of(self.from),
            origin: self.origin,
            n: self.n,
        }
    }
}

impl Residency<TerrainTile> {
    /// The lowest and highest the ground holds over the column `node`'s own
    /// cells, in leaf cells: the bound over the cells of `line`'s tile, off
    /// the finest window holding them, one look-up whatever the node's size
    /// (`relief`). A coarser window standing in holds its own cells, which
    /// its bound holds, and names the tile lacked. None for a column a shard
    /// or finer past the service area, where no tile is; where no window
    /// holds it, the lack, never nothing.
    pub fn relief(&self, line: &Line, node: Node<2>) -> Answer<Option<[i32; 2]>> {
        if node.0 >= SHARD_ZOOM && past(node) {
            return Answer::Exact(None);
        }
        let (r, from, here) = relief(line.tile, node);
        held(line, from, here, Some(r))
    }

    /// Whether the windows hold the cells of `line`'s tile under `node` as
    /// its line says them, each square of them read (`held`).
    pub fn cells_held(&self, line: &Line, (zoom, at): Node<2>) -> Answer<()> {
        let cell = GROUND.cell_zoom(line.tile.zoom);
        let [lo, hi] = match cell.checked_sub(zoom) {
            Some(d) => [at.map(|v| v << d), at.map(|v| ((v + 1) << d) - 1)],
            None => [at.map(|v| v >> (zoom - cell)); 2],
        }
        .map(|c| c.map(|v| (v >> SQUARE_SHIFT) as i32));
        let mut coarsest = cell;
        for y in lo[1]..=hi[1] {
            for x in lo[0]..=hi[0] {
                let (from, _, _, here) = cells::find(&Windowed, cell, [x << SQUARE_SHIFT, y << SQUARE_SHIFT], FIRST_CELL);
                if !here {
                    return Answer::Nothing(line.tile);
                }
                coarsest = coarsest.min(from);
            }
        }
        held(line, coarsest, true, ())
    }

    /// The cells of the tile `line` walked to as the resident tile it is
    /// drawn from holds them; none where none is held.
    pub fn source_of(&self, line: &Line) -> Option<Source<'_>> {
        let from = line.drawn?;
        let corner = GROUND.cell_at(line.tile, GROUND.cell_index(0, 0));
        Some(Source {
            tile: self.get(from)?,
            from,
            origin: corner.in_tile_above(line.tile.zoom - from.zoom),
            n: count(GROUND.cell_zoom(line.tile.zoom), GROUND.cell_zoom(from.zoom)),
        })
    }
}

/// The tile a node's bound is read from: its cells' tile, within the
/// pyramid's zooms.
fn bound_tile<const D: usize>((zoom, at): Node<D>, cell: Zoom) -> Tile {
    let read = GROUND.read_zoom(cell);
    tile_at(read, ancestor_of([at[0], at[1]], zoom - read))
}

/// `v`, read off the window at `from` (`here`, else none held): exact where
/// `line` says the ground at `from` or coarser, else coarser, naming its tile.
fn held<T>(line: &Line, from: Zoom, here: bool, v: T) -> Answer<T> {
    match here {
        false => Answer::Nothing(line.tile),
        true if line.source.is_some_and(|s| from >= GROUND.cell_zoom(s.zoom)) => Answer::Exact(v),
        true => Answer::Coarser(v, line.tile),
    }
}

/// The bound under a node over the cells of `tile`, off the finest window
/// holding them (pxl-portable `window::bounded`), and that window's zoom.
fn relief(tile: Tile, (zoom, at): Node<2>) -> ([i32; 2], Zoom, bool) {
    cells::bounded(&Windowed, GROUND.cell_zoom(tile.zoom), (zoom, at.map(|v| v as i32)), FIRST_CELL)
}

/// A node's center, in units.
fn center_of((zoom, at): Node<2>) -> [f64; 2] {
    let base = base_of(zoom, [at[0], at[1], 0]);
    [i64::from(base[0]), i64::from(base[1])].map(|v| q3::to_real(v) + units(zoom) / 2.0)
}

/// Whether a node is past the service area. It never straddles the area's
/// edge, which runs on shards' edges.
fn past(node: Node<2>) -> bool {
    source::past_at(center_of(node))
}

/// The look of the cell of `tile` at a node's center, off the finest window
/// holding it; void where none does.
fn look_at(tile: Tile, node: Node<2>) -> u32 {
    let [x, y] = center_of(node);
    let cell = GROUND.cell_zoom(tile.zoom);
    let g = GROUND.cell_of(tile.zoom, &point(x, y, 0.0)).node().1.map(|v| v as i32);
    match cells::find(&Windowed, cell, g, FIRST_CELL) {
        (from, slot, i, true) => pxl_portable::ground::own(Windowed.look(from, slot, i), from),
        _ => u32::from(pxl_spec::block::cube::sentinel::void),
    }
}

/// The ring of cells at `fine` about a node, or of nodes where it is finer.
fn ring(fine: Zoom, (zoom, at): Node<2>) -> impl Iterator<Item = Node<2>> {
    let ring = fine.max(zoom);
    let k = count(ring, zoom) as i64;
    let (x0, y0, rows) = (at[0] * k, at[1] * k, 1i64 << ring);
    (y0 - 1..=y0 + k).filter(move |y| (0..rows).contains(y)).flat_map(move |y| {
        let step = if y == y0 - 1 || y == y0 + k { 1 } else { k + 1 };
        (0..).map(move |n| x0 - 1 + n * step).take_while(move |&x| x <= x0 + k).map(move |x| (ring, [x.rem_euclid(rows), y]))
    })
}

impl Query for Residency<TerrainTile> {
    type Record<'a> = Source<'a>;

    type Cursor = Line;

    fn cursor<const D: usize>(&self, node: Node<D>, cell: Zoom) -> Line {
        self.line(bound_tile(node, cell))
    }

    /// The tile a child reads is its parent's, or one of its children.
    fn child<const D: usize>(&self, parent: &Line, node: Node<D>, cell: Zoom) -> Line {
        match bound_tile(node, cell) {
            tile if tile == parent.tile => *parent,
            tile if tile.zoom == parent.tile.zoom + 1 && tile.parent() == parent.tile => self.step(parent, tile),
            tile => self.line(tile),
        }
    }

    /// From the bound of the tile it is drawn from: the ground that can
    /// show, down to a cell at `cell` under the lowest face about the node,
    /// its own or beside, under which every column is solid; so a step down to
    /// a column beside is walled, and a fold's cell shrunk by its fill stands
    /// on the cell under it, never on the sky. A
    /// node past the service area, no look in it, holds nothing; one beside
    /// it holds the bedrock walling the area from inside down to its floor
    /// (`pxl-spec block/sample.rs` `wall_off`), so the wall is seen whole.
    fn bound<const D: usize>(&self, line: &Line, (zoom, at): Node<D>, cell: Zoom) -> Answer<Option<Bound>> {
        let node = (zoom, [at[0], at[1]]);
        // Known before any of its ground arrives.
        if past(node) {
            return Answer::Exact(None);
        }
        let ([mut lo, hi], lack) = match self.relief(line, node) {
            Answer::Exact(r) => (r.unwrap_or_default(), None),
            Answer::Coarser(r, t) => (r.unwrap_or_default(), Some(t)),
            Answer::Nothing(t) => return Answer::Nothing(t),
        };
        let mut lack = lack;
        let mut edge = false;
        let mut last: Option<Line> = None;
        // The apron is a column of the chunk's cells, or the tile's where coarser.
        for beside in ring(GROUND.cell_zoom(line.tile.zoom).min(cell), node) {
            if past(beside) {
                edge = true;
                continue;
            }
            let tile = tile_at(line.tile.zoom, ancestor_of(beside.1, beside.0 - line.tile.zoom));
            let b = match last {
                Some(b) if b.tile == tile => b,
                _ if tile == line.tile => *line,
                _ => self.line(tile),
            };
            last = Some(b);
            // Inside the area, as just asked.
            let (r, from, here) = relief(tile, beside);
            let [b_lo, _] = match held(&b, from, here, r) {
                Answer::Nothing(t) => {
                    lack = lack.or(Some(t));
                    continue;
                }
                Answer::Coarser(r, t) => {
                    lack = lack.or(Some(t));
                    r
                }
                Answer::Exact(r) => r,
            };
            if Ground::grounds(look_at(tile, beside)) {
                lo = lo.min(b_lo);
            }
        }
        if edge {
            lo = lo.min(bedrock(lo));
        }
        let r = Relief {
            extent: (f64::from(lo) * units(GROUND.cell()) - units(cell), f64::from(hi) * units(GROUND.cell())),
        };
        let r = lack.map_or(Answer::Exact(r), |t| Answer::Coarser(r, t));
        let whole = region((zoom, at));
        r.map(|r| {
            let (lo, hi) = (r.extent.0.max(whole.min[2]), r.extent.1.min(whole.max[2]));
            (lo <= hi).then(|| Bound {
                aabb: pxl_spec::space::coord::aabb::Aabb {
                    min: [whole.min[0], whole.min[1], lo],
                    max: [whole.max[0], whole.max[1], hi],
                },
                finest: GROUND.cell(),
            })
        })
    }

    /// The lines over the node's tile, as `held` says the tile it is drawn
    /// from: exact where a resident tile says it, else coarser.
    fn read(&self, node: Node<3>, cell: Zoom) -> Answer<Source<'_>> {
        let line = self.cursor(node, cell);
        match self.source_of(&line) {
            Some(s) => held(&line, GROUND.cell_zoom(s.from.zoom), true, s),
            None => Answer::Nothing(line.tile),
        }
    }
}
