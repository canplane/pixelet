// pxl-kernel/layer/terrain/query.rs

//! Terrain's answers. A node's ground is read from the tile its cells fold
//! from, or the nearest resident tile over it, at that tile's cells; it is
//! exact where that tile says it, coarser where one over it only draws it.

use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::block::cube::Ground;
use pxl_spec::block::limit::bedrock;
use pxl_spec::block::sample::{TileSource, TileView};
use pxl_spec::block::stand::z_unit;
use pxl_spec::procedural::Frame;
use pxl_spec::space::coord::pvf3::pvf3;
use pxl_spec::space::coord::zoom::{base_of, count, units, Zoom};
use pxl_spec::space::hierarchy::tree::{ancestor_of, region, Node};
use pxl_spec::space::layer::lattice::{EXTENT, GROUND};
use pxl_spec::space::layer::tile::{tile_at, Tile};

use crate::layer::residency::{Line, Residency};

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
            tile: tile_view(self.tile),
            frame: Frame::of(self.from),
            origin: self.origin,
            n: self.n,
        }
    }
}

pub fn tile_view(tile: &TerrainTile) -> TileView<'_> {
    TileView {
        cells: &tile.ground.cells,
        ele: &tile.ele,
        separation: &tile.separation,
    }
}

/// A tile as the resident tile it is drawn from answers it.
#[derive(Clone, Copy)]
pub(crate) struct Drawn<'a> {
    tile: Tile,
    pub from: Tile,
    pub body: &'a TerrainTile,
    exact: bool,
}

impl<'a> Drawn<'a> {
    /// Its tile's cells as they are read from the tile it is drawn from.
    pub(crate) fn source(&self) -> Source<'a> {
        let corner = GROUND.cell_at(self.tile, GROUND.cell_index(0, 0));
        Source {
            tile: self.body,
            from: self.from,
            origin: corner.in_tile_above(self.tile.zoom - self.from.zoom),
            n: count(GROUND.cell_zoom(self.tile.zoom), GROUND.cell_zoom(self.from.zoom)),
        }
    }

    /// Exact where a resident tile says the tile's ground, else coarser.
    pub(crate) fn answer<T>(&self, v: T) -> Answer<T> {
        match self.exact {
            true => Answer::Exact(v),
            false => Answer::Coarser(v, self.tile),
        }
    }
}

impl Residency<TerrainTile> {
    /// The resident tile `tile` is drawn from and its body, and whether it
    /// says `tile` exactly: one walk of `tile`'s line.
    pub(crate) fn drawn(&self, tile: Tile) -> Option<Drawn<'_>> {
        self.drawn_of(&self.line(tile))
    }

    /// As `drawn`, its tile's line already walked.
    pub(crate) fn drawn_of(&self, line: &Line) -> Option<Drawn<'_>> {
        let from = line.drawn?;
        Some(Drawn {
            tile: line.tile,
            from,
            body: self.get(from)?,
            exact: line.source.is_some(),
        })
    }
}

/// Cells of ground drawn under the lowest face about a node, its own or a
/// column's beside it, so a step down to a neighbour is walled.
const DRAWN_UNDER: f64 = 4.0;

/// How far under the lowest face about a node its chunks are drawn, in
/// units, for cells at `cell`: past the leaves a fold's whole cell too, so a
/// cell shrunk by its fill stands on ground, never on the sky. A chunk
/// composes whatever heights it is; this decides which are drawn.
fn drawn_under(cell: Zoom) -> f64 {
    DRAWN_UNDER * z_unit(cell) + units(cell)
}

/// The tile a node's bound is read from: its cells' tile, within the
/// pyramid's zooms.
fn bound_tile<const D: usize>((zoom, at): Node<D>, cell: Zoom) -> Tile {
    let read = GROUND.read_zoom(cell);
    tile_at(read, ancestor_of([at[0], at[1]], u32::from(zoom - read)))
}

/// The index lane's extent over a node within the tile `d` answers, widened
/// by what is drawn past it (`DRAWN_PAST`).
fn relief(d: &Drawn, (zoom, at): Node<2>) -> [i16; 2] {
    let index = EXTENT.cell_zoom(d.from.zoom);
    let first = EXTENT.cell_of(d.from.zoom, &base_of(zoom, [at[0], at[1], 0]));
    let across = if zoom < index { count(index, zoom) } else { 1 };
    d.body.index.over(first.in_tile(), across)
}

/// Whether the cell at a node's center has a look: the service area's ground.
/// A node never straddles the area's edge, which runs on shards' edges.
fn has_look(d: &Drawn, (zoom, at): Node<2>) -> bool {
    let base = base_of(zoom, [at[0], at[1], 0]);
    let half = pvf3::from_real(units(zoom) / 2.0);
    let center = pvf3::gather(pvf3::X(&base) + half, pvf3::Y(&base) + half, 0);
    let [x, y] = GROUND.cell_of(d.from.zoom, &center).in_tile();
    Ground::of(d.body.ground.cells[GROUND.cell_index(x, y) as usize]).is_some()
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

    /// From the index lane of the tile it is drawn from, about the node as
    /// its columns are drawn beside theirs, and down as far as its ground is
    /// drawn (`drawn_under`). For deciding descent, never for what is drawn. A
    /// node past the service area, no look in it, holds the bedrock walling
    /// the area where ground is beside it (`pxl-spec block/limit.rs` `wall`):
    /// from the bedrock to that ground's face.
    fn bound<const D: usize>(&self, line: &Line, (zoom, at): Node<D>, cell: Zoom) -> Answer<Option<Bound>> {
        let Some(d) = self.drawn_of(line) else {
            return Answer::Nothing(line.tile);
        };
        let node = (zoom, [at[0], at[1]]);
        let [mut lo, mut hi] = relief(&d, node).map(i32::from);
        let mut lack = (!d.exact).then_some(d.tile);
        let inside = has_look(&d, node);
        let mut face = None;
        let mut last: Option<(Tile, Option<Drawn>)> = None;
        // The apron is a column of the chunk's cells; the index lane says no finer.
        for beside in ring(EXTENT.cell_zoom(line.tile.zoom).min(cell), node) {
            let tile = tile_at(line.tile.zoom, ancestor_of(beside.1, u32::from(beside.0 - line.tile.zoom)));
            let b = match last {
                Some((t, b)) if t == tile => b,
                _ if tile == line.tile => Some(d),
                _ => self.drawn(tile),
            };
            last = Some((tile, b));
            let Some(b) = b else {
                lack = lack.or(Some(tile));
                continue;
            };
            lack = lack.or((!b.exact).then_some(b.tile));
            if has_look(&b, beside) {
                let [b_lo, b_hi] = relief(&b, beside).map(i32::from);
                match inside {
                    true => lo = lo.min(b_lo),
                    false => face = face.max(Some(b_hi)),
                }
            }
        }
        if let Some(face) = face {
            (lo, hi) = (lo.min(bedrock(face, 1)), hi.max(face));
        }
        let r = Relief {
            extent: (f64::from(lo) * units(GROUND.cell()) - drawn_under(cell), f64::from(hi) * units(GROUND.cell())),
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

    fn read(&self, (zoom, [x, y, _]): Node<3>, cell: Zoom) -> Answer<Source<'_>> {
        let p = GROUND.read_tile((zoom, [x, y]), cell);
        let Some(d) = self.drawn(p) else { return Answer::Nothing(p) };
        d.answer(d.source())
    }
}
