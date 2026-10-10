// pxl-kernel/layer/terrain/query.rs

//! Terrain's answers. A node's ground is read from the tile its cells fold
//! from, or the nearest resident tile over it, at that tile's cells; it is
//! exact where that tile says it, coarser where one over it only draws it.

use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::block::sample::{TileSource, TileView};
use pxl_spec::procedural::Frame;
use pxl_spec::space::coord::zoom::{base_of, count, units, Zoom};
use pxl_spec::space::hierarchy::tree::{ancestor_of, region, Node};
use pxl_spec::space::layer::lattice::{EXTENT, GROUND};
use pxl_spec::space::layer::tile::{tile_at, Tile, LEAF_ZOOM};

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

/// The tile a node's bound is read from: its cells' tile, within the
/// pyramid's zooms.
fn bound_tile<const D: usize>((zoom, at): Node<D>, cell: Zoom) -> Tile {
    let read = GROUND.tile_zoom_of_grid(cell).clamp(i32::from(GROUND.top()), i32::from(LEAF_ZOOM)) as Zoom;
    tile_at(read, ancestor_of([at[0], at[1]], u32::from(zoom - read)))
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

    /// From one block of the index lane of the tile it is drawn from. For
    /// deciding descent, never for what is drawn.
    fn bound<const D: usize>(&self, line: &Line, (zoom, at): Node<D>, cell: Zoom) -> Answer<Option<Bound>> {
        let _ = cell;
        let Some(d) = self.drawn_of(line) else {
            return Answer::Nothing(line.tile);
        };
        let (src, body) = (d.from, d.body);
        let index = EXTENT.cell_zoom(src.zoom);
        let first = EXTENT.cell_of(src.zoom, &base_of(zoom, [at[0], at[1], 0]));
        let across = if zoom < index { count(index, zoom) } else { 1 };
        let [lo, hi] = body.index.over(first.in_tile(), across);
        let r = Relief {
            extent: (f64::from(lo) * units(GROUND.cell()), f64::from(hi) * units(GROUND.cell())),
        };
        let r = d.answer(r);
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
