// pxl-kernel/world/access/terrain.rs

//! Terrain's answers. A node's ground is read from the tile its cells fold
//! from, or the nearest resident tile over it, at that tile's cells; it is
//! exact where that tile says it, coarser where one over it only draws it.

use pxl_render::mesh::paint::terrain::adapter::{TileSource, TileView};
use pxl_spec::bake::format::Terrain;
use pxl_spec::procedural::Frame;
use pxl_spec::space::coord::zoom::{base_of, count, cube, units, Zoom};
use pxl_spec::space::hierarchy::tree::{ancestor_of, region, Node};
use pxl_spec::space::layer::lattice::{EXTENT, GROUND};
use pxl_spec::space::layer::tile::{tile_at, Tile, LEAF_ZOOM};

use crate::world::residency::Residency;

use super::answer::{Answer, Relief};
use super::query::{Bound, Query};

/// Where a node's cells are read from: the tile that answered, and where the
/// first cell of the tile asked lies in it, `n` of the asked tile's cells to
/// one of its own.
#[derive(Clone, Copy)]
pub struct Source<'a> {
    pub tile: &'a Terrain,
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

pub fn tile_view(tile: &Terrain) -> TileView<'_> {
    TileView {
        cells: &tile.ground.cells,
        ele: &tile.ele,
        decks: &tile.decks,
    }
}

impl Residency<Terrain> {
    /// The resident tile `tile` is drawn from, and its body.
    pub(crate) fn drawn(&self, tile: Tile) -> Option<(Tile, &Terrain)> {
        let src = self.drawn_from(tile)?;
        Some((src, self.get(src)?))
    }

    /// Exact where a resident tile says `tile`'s ground, else coarser.
    pub(crate) fn answer<T>(&self, tile: Tile, v: T) -> Answer<T> {
        match self.source(tile) {
            Some(_) => Answer::Exact(v),
            None => Answer::Coarser(v, tile),
        }
    }

    /// Bound on the ground over a node of `tile`'s zoom and every zoom finer than
    /// it, from one block of the index lane. For deciding descent, never for
    /// what is drawn.
    pub fn relief(&self, tile: Tile, (side, at): Node<2>) -> Answer<Relief> {
        let Some((src, body)) = self.drawn(tile) else {
            return Answer::Nothing(tile);
        };
        let index = EXTENT.cell_zoom(src.zoom);
        let first = EXTENT.cell_of(src.zoom, &base_of(side, [at[0], at[1], 0]));
        let across = if side < index { count(index, side) } else { 1 };
        let b = body.index.over(first.in_tile(), across);
        self.answer(
            tile,
            Relief {
                extent: (f64::from(b.lo) * units(GROUND.cell()), f64::from(b.hi) * units(GROUND.cell())),
            },
        )
    }
}

impl Query for Residency<Terrain> {
    type Record<'a> = Source<'a>;

    fn bounds<const D: usize>(&self, (zoom, at): Node<D>, cell: Zoom) -> Answer<Option<Bound>> {
        let [x, y] = [at[0], at[1]];
        let read = GROUND.tile_zoom_of_grid(cell).clamp(i32::from(GROUND.top()), i32::from(LEAF_ZOOM)) as Zoom;
        let tile = tile_at(read, ancestor_of([x, y], u32::from(zoom - read)));
        let whole = region((zoom, at));
        self.relief(tile, (zoom, [x, y])).map(|r| {
            let (lo, hi) = (r.extent.0.max(whole.min[2]), r.extent.1.min(whole.max[2]));
            (lo <= hi).then(|| Bound {
                aabb: pxl_spec::space::coord::v3::Aabb {
                    min: [whole.min[0], whole.min[1], lo],
                    max: [whole.max[0], whole.max[1], hi],
                },
                finest: GROUND.cell(),
            })
        })
    }

    fn read(&self, (zoom, [x, y, _]): Node<3>, cell: Zoom) -> Answer<Source<'_>> {
        let p = GROUND.read_tile((zoom, [x, y]), cell);
        let Some((src, tile)) = self.drawn(p) else { return Answer::Nothing(p) };
        let corner = GROUND.cell_at(p, GROUND.cell_index(0, 0));
        self.answer(
            p,
            Source {
                tile,
                from: src,
                origin: corner.in_tile_above(p.zoom - src.zoom),
                n: count(GROUND.cell_zoom(p.zoom), GROUND.cell_zoom(src.zoom)),
            },
        )
    }
}
