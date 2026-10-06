// pxl-kernel/world/pyramid/chunk.rs

//! A chunk: a cube of 64 cells of one zoom, the grain a view is drawn at,
//! addressed by its own zoom, and drawn in cells a step coarser
//! (`DRAWN_STEPS`). Never fetched or held; only its address exists, and an
//! empty one is never made. It reads the tile whose cells are the ones it
//! draws, or past the leaves the leaf over it, whose cells it divides; its
//! tile and place in the tile derive from its column, so they cannot
//! disagree with where it is.

use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::space::hierarchy::{ancestor_of, child_of, offset_in, parent_of};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pyramid::{chunk_zoom, tile_at, tile_rows, wrap_cell, Tile, CHUNK_DRAWN, CHUNK_STEPS, DRAWN_STEPS, GROUND, LEAF_Z, TILE_CELLS, TOP_Z};
use pxl_spec::space::v3::Aabb;
use pxl_spec::space::zoom::{base_of, index_of, units, Zoom};

pub const CHILDREN: usize = 8;

/// The coarsest chunk: the top tile's.
pub const TOP_CHUNK: Zoom = chunk_zoom(TOP_Z);

/// The leaves' chunk: the finest whose drawn cells are a tile's.
pub const LEAF_CHUNK: Zoom = chunk_zoom(LEAF_Z);

/// The finest chunk: it draws the objects' finest cells.
pub const FINEST_CHUNK: Zoom = OBJECT_FINEST + DRAWN_STEPS - CHUNK_STEPS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Chunk {
    zoom: Zoom,
    at: [i64; 3],
}

impl Chunk {
    /// The chunk at `zoom` holding `at`.
    pub fn holding(zoom: Zoom, at: &Pvf3) -> Chunk {
        Chunk { zoom, at: index_of(zoom, at) }
    }

    pub fn indexed(zoom: Zoom, at: [i64; 3]) -> Chunk {
        Chunk { zoom, at }
    }

    /// As the bounds of a range in chunk order.
    pub fn column_holding(zoom: Zoom, at: &Pvf3) -> std::ops::RangeInclusive<Chunk> {
        let [x, y, _] = index_of(zoom, at);
        Chunk { zoom, at: [x, y, i64::MIN] }..=Chunk { zoom, at: [x, y, i64::MAX] }
    }

    pub fn beside(self, dx: i64, dy: i64, dh: i64) -> Chunk {
        let [x, y, h] = self.at;
        Chunk {
            zoom: self.zoom,
            at: [x + dx, y + dy, h + dh],
        }
    }

    pub fn zoom(self) -> Zoom {
        self.zoom
    }

    /// Its cells': a chunk is 64 of them a side, the cells the LOD rule measures.
    pub fn cell_zoom(self) -> Zoom {
        self.zoom + CHUNK_STEPS
    }

    /// The cells it is drawn in, `CHUNK_DRAWN` a side.
    pub fn drawn_zoom(self) -> Zoom {
        self.cell_zoom() - DRAWN_STEPS
    }

    /// The tile it reads: the one whose cells it draws, or the leaf over it.
    pub fn tile_zoom(self) -> Zoom {
        (self.zoom - (LEAF_CHUNK - LEAF_Z)).min(LEAF_Z)
    }

    /// Zooms its drawn cells are finer than its tile's: none but past the leaves.
    pub fn finer(self) -> u32 {
        u32::from(self.drawn_zoom() - GROUND.cell_zoom(self.tile_zoom()))
    }

    /// Its tile's side in its drawn cells.
    fn tile_span(self) -> u32 {
        TILE_CELLS << self.finer()
    }

    /// The lane's copy, as `holding` makes it: one name for every round of the world.
    pub fn named(self) -> Chunk {
        Chunk {
            zoom: self.zoom,
            at: wrap_cell(self.zoom, self.at),
        }
    }

    pub fn tile(self) -> Tile {
        tile_at(self.tile_zoom(), ancestor_of(self.column().1, u32::from(self.zoom - self.tile_zoom())))
    }

    /// Its first drawn cell's place in its tile, in drawn cells.
    pub fn in_tile(self) -> (u32, u32) {
        let [x, y] = offset_in(self.column().1, u32::from(self.zoom - self.tile_zoom()));
        (x as u32 * CHUNK_DRAWN, y as u32 * CHUNK_DRAWN)
    }

    /// Neighbouring tiles an apron of `apron` of its drawn cells reaches into, with
    /// their offsets from the chunk's tile; none for a chunk inside it.
    pub fn tiles_beside(self, apron: u32) -> impl Iterator<Item = ([i64; 2], Tile)> {
        let (x0, y0) = self.in_tile();
        let span = self.tile_span();
        let steps = move |v: u32| [0, -1, 1].into_iter().filter(move |&q| q == 0 || (q < 0 && v < apron) || (q > 0 && v + CHUNK_DRAWN + apron > span));
        let tile = self.tile();
        let [px, py] = [ivec::X(tile.tile), ivec::Y(tile.tile)];
        let zoom = self.tile_zoom();
        let rows = tile_rows(zoom);
        steps(y0)
            .flat_map(move |qy| steps(x0).map(move |qx| [qx, qy]))
            .filter(move |&[qx, qy]| [qx, qy] != [0, 0] && rows.contains(&(py + qy)))
            .map(move |[qx, qy]| ([qx, qy], tile_at(zoom, [px + qx, py + qy])))
    }

    /// The corner at no height, as the 2D stores key it; where it stands is `rect`'s.
    pub fn corner(self) -> IVec {
        let [x, y, _] = self.at;
        base_of(self.zoom, [x, y, 0]).i
    }

    /// From the index, which runs on round the world as the view's frame does.
    pub fn rect(self) -> (f64, f64, f64) {
        let span = units(self.zoom);
        (self.at[0] as f64 * span, self.at[1] as f64 * span, span)
    }

    /// From the index, not the corner point: a point's height is 16 bits
    /// (±32,768 units) and the top chunk is 65,536 tall.
    pub fn z_range(self) -> (f64, f64) {
        let span = units(self.zoom);
        let z0 = self.at[2] as f64 * span;
        (z0, z0 + span)
    }

    pub fn region(self) -> Aabb {
        let span = units(self.zoom);
        let (x0, y0, _) = self.rect();
        let z0 = self.z_range().0;
        Aabb {
            min: [x0, y0, z0],
            max: [x0 + span, y0 + span, z0 + span],
        }
    }

    pub fn node(self) -> (Zoom, [i64; 3]) {
        (self.zoom, self.at)
    }

    pub fn column(self) -> (Zoom, [i64; 2]) {
        let [x, y, _] = self.at;
        (self.zoom, [x, y])
    }

    pub fn parent(self) -> Option<Chunk> {
        (self.zoom > TOP_CHUNK).then(|| Chunk {
            zoom: self.zoom - 1,
            at: parent_of(self.at),
        })
    }

    pub fn ancestors(self) -> impl Iterator<Item = Chunk> {
        std::iter::successors(self.parent(), |k| k.parent())
    }

    /// In `slot_of`'s order.
    pub fn children(self) -> [Chunk; CHILDREN] {
        std::array::from_fn(|slot| Chunk {
            zoom: self.zoom + 1,
            at: child_of(self.at, slot),
        })
    }
}
