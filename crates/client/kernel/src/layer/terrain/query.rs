// pxl-kernel/layer/terrain/query.rs

//! Terrain's answers. A node's ground is read from the tile its cells fold
//! from, or the nearest resident tile over it, at that tile's cells; it is
//! exact where that tile says it, coarser where one over it only draws it.

use pxl_portable::ground::Tile as _;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::block::cube::Ground;
use pxl_spec::block::limit::bedrock;
use pxl_spec::block::sample::{TileSource, TileView};
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
        heights: &tile.heights,
        ways: &tile.ways,
        levels: &tile.levels,
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

    /// The lowest and highest the ground holds over the column `node`'s own
    /// cells, in leaf cells: the bound over the cells of the tile `line`
    /// draws it from, one look-up
    /// whatever the node's size (`relief`). A coarser tile standing in draws
    /// its own cells, which its bound holds, and names the tile lacked. None
    /// for a column a shard or finer past the service area, where no tile
    /// is; where none is held, the lack, never nothing.
    pub fn relief(&self, line: &Line, node: Node<2>) -> Answer<Option<[i32; 2]>> {
        if node.0 >= SHARD_ZOOM && past(node) {
            return Answer::Exact(None);
        }
        let Some(d) = self.drawn_of(line) else {
            return Answer::Nothing(line.tile);
        };
        let r = Some(relief(&d, node));
        match d.exact {
            true => Answer::Exact(r),
            false => Answer::Coarser(r, d.tile),
        }
    }

    /// The cells of the tile `line` walked to, read off the tile it is drawn
    /// from: exact where a resident tile says it, coarser where one over it
    /// only draws it, the lack where none is held.
    pub fn cells_of(&self, line: &Line) -> Answer<Source<'_>> {
        let Some(d) = self.drawn_of(line) else { return Answer::Nothing(line.tile) };
        d.answer(d.source())
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

/// The tile a node's bound is read from: its cells' tile, within the
/// pyramid's zooms.
fn bound_tile<const D: usize>((zoom, at): Node<D>, cell: Zoom) -> Tile {
    let read = GROUND.read_zoom(cell);
    tile_at(read, ancestor_of([at[0], at[1]], zoom - read))
}

/// The bound over the cells of the tile `d` answers under a node
/// (pxl-portable `ground::bounded`).
fn relief(d: &Drawn, (zoom, at): Node<2>) -> [i32; 2] {
    pxl_portable::ground::bounded(&tile_view(d.body), d.from.zoom, (zoom, at.map(|v| v as i32)))
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

/// The look of the cell at a node's center in the tile `d` answers.
fn look_at(d: &Drawn, node: Node<2>) -> u32 {
    let [x, y] = center_of(node);
    let [x, y] = GROUND.cell_of(d.from.zoom, &point(x, y, 0.0)).in_tile();
    pxl_portable::ground::own(d.source().view().tile.look(x, y), GROUND.cell_zoom(d.from.zoom))
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
        let Some(d) = self.drawn_of(line) else {
            return Answer::Nothing(line.tile);
        };
        let [mut lo, hi] = relief(&d, node).map(i32::from);
        let mut lack = (!d.exact).then_some(d.tile);
        let mut edge = false;
        let mut last: Option<(Tile, Option<Drawn>)> = None;
        // The apron is a column of the chunk's cells, or the tile's where coarser.
        for beside in ring(GROUND.cell_zoom(line.tile.zoom).min(cell), node) {
            if past(beside) {
                edge = true;
                continue;
            }
            let tile = tile_at(line.tile.zoom, ancestor_of(beside.1, beside.0 - line.tile.zoom));
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
            if Ground::grounds(look_at(&b, beside)) {
                lo = lo.min(relief(&b, beside)[0]);
            }
        }
        if edge {
            lo = lo.min(bedrock(lo, 1));
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

    fn read(&self, node: Node<3>, cell: Zoom) -> Answer<Source<'_>> {
        self.cells_of(&self.cursor(node, cell))
    }
}
