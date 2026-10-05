// pxl-kernel/world/pyramid/chunk.rs

//! A chunk: a cube of 64 cells of one zoom, the grain a view is drawn at.
//! Never fetched or held; only its address exists, and an empty one is never
//! made. Its tile and place in the tile derive from its column, so they
//! cannot disagree with where it is.

use pxl_spec::space::hierarchy::{ancestor_of, child_of, offset_in, parent_of};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pyramid::{chunk_zoom, tile_at, tile_rows, wrap_cell, Tile, CHUNK_CELLS, LEAF_Z, TILE_CELLS, TOP_Z};
use pxl_spec::space::v3::Aabb;
use pxl_spec::space::zoom::{base_of, index_of, units, Zoom};

pub const CHILDREN: usize = 8;

/// A tile is this many steps over a chunk.
const TILE_STEPS: u32 = (chunk_zoom(LEAF_Z) - LEAF_Z) as u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Chunk {
    tile_zoom: Zoom,
    at: [i64; 3],
}

impl Chunk {
    pub fn of(z: Zoom, at: &Pvf3) -> Chunk {
        Chunk {
            tile_zoom: z,
            at: index_of(chunk_zoom(z), at),
        }
    }

    pub fn at(z: Zoom, at: [i64; 3]) -> Chunk {
        Chunk { tile_zoom: z, at }
    }

    /// As the bounds of a range in chunk order.
    pub fn column_of(z: Zoom, at: &Pvf3) -> std::ops::RangeInclusive<Chunk> {
        let [x, y, _] = index_of(chunk_zoom(z), at);
        Chunk { tile_zoom: z, at: [x, y, i64::MIN] }..=Chunk { tile_zoom: z, at: [x, y, i64::MAX] }
    }

    pub fn beside(self, dx: i64, dy: i64, dh: i64) -> Chunk {
        let [x, y, h] = self.at;
        Chunk {
            tile_zoom: self.tile_zoom,
            at: [x + dx, y + dy, h + dh],
        }
    }

    pub fn tile_zoom(self) -> Zoom {
        self.tile_zoom
    }

    /// The lane's copy, as `of` makes it: one name for every round of the world.
    pub fn named(self) -> Chunk {
        Chunk {
            tile_zoom: self.tile_zoom,
            at: wrap_cell(chunk_zoom(self.tile_zoom), self.at),
        }
    }

    pub fn tile(self) -> Tile {
        tile_at(self.tile_zoom, ancestor_of(self.column().1, TILE_STEPS))
    }

    pub fn in_tile(self) -> (u32, u32) {
        let [x, y] = offset_in(self.column().1, TILE_STEPS);
        (x as u32 * CHUNK_CELLS, y as u32 * CHUNK_CELLS)
    }

    /// Neighbouring tiles an apron of `apron` cells reaches into, with their
    /// offsets from the chunk's tile; none for a chunk inside it.
    pub fn tiles_beside(self, apron: u32) -> impl Iterator<Item = ([i64; 2], Tile)> {
        let (x0, y0) = self.in_tile();
        let steps = move |v: u32| {
            [0, -1, 1]
                .into_iter()
                .filter(move |&q| q == 0 || (q < 0 && v < apron) || (q > 0 && v + CHUNK_CELLS + apron > TILE_CELLS))
        };
        let tile = self.tile();
        let [px, py] = [ivec::X(tile.tile), ivec::Y(tile.tile)];
        let (z, rows) = (self.tile_zoom, tile_rows(self.tile_zoom));
        steps(y0)
            .flat_map(move |qy| steps(x0).map(move |qx| [qx, qy]))
            .filter(move |&[qx, qy]| [qx, qy] != [0, 0] && rows.contains(&(py + qy)))
            .map(move |[qx, qy]| ([qx, qy], tile_at(z, [px + qx, py + qy])))
    }

    /// The corner at no height, as the 2D stores key it; where it stands is `rect`'s.
    pub fn corner(self) -> IVec {
        let [x, y, _] = self.at;
        base_of(chunk_zoom(self.tile_zoom), [x, y, 0]).i
    }

    /// From the index, which runs on round the world as the view's frame does.
    pub fn rect(self) -> (f64, f64, f64) {
        let span = units(chunk_zoom(self.tile_zoom));
        (self.at[0] as f64 * span, self.at[1] as f64 * span, span)
    }

    /// From the index, not the corner point: a point's height is 16 bits
    /// (±32,768 units) and a z5 chunk is 65,536 tall.
    pub fn z_range(self) -> (f64, f64) {
        let span = units(chunk_zoom(self.tile_zoom));
        let z0 = self.at[2] as f64 * span;
        (z0, z0 + span)
    }

    pub fn region(self) -> Aabb {
        let span = units(chunk_zoom(self.tile_zoom));
        let (x0, y0, _) = self.rect();
        let z0 = self.z_range().0;
        Aabb {
            min: [x0, y0, z0],
            max: [x0 + span, y0 + span, z0 + span],
        }
    }

    pub fn node(self) -> (Zoom, [i64; 3]) {
        (chunk_zoom(self.tile_zoom), self.at)
    }

    pub fn column(self) -> (Zoom, [i64; 2]) {
        let [x, y, _] = self.at;
        (chunk_zoom(self.tile_zoom), [x, y])
    }

    pub fn parent(self) -> Option<Chunk> {
        (self.tile_zoom > TOP_Z).then(|| Chunk {
            tile_zoom: self.tile_zoom - 1,
            at: parent_of(self.at),
        })
    }

    pub fn ancestors(self) -> impl Iterator<Item = Chunk> {
        std::iter::successors(self.parent(), |k| k.parent())
    }

    /// In `slot_of`'s order.
    pub fn children(self) -> [Chunk; CHILDREN] {
        std::array::from_fn(|slot| Chunk {
            tile_zoom: self.tile_zoom + 1,
            at: child_of(self.at, slot),
        })
    }
}
