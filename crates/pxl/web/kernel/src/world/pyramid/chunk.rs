// pxl-kernel/world/pyramid/chunk.rs

//! A chunk: a cube of the camera grid's cells, `CHUNK_STEPS` zooms finer than
//! itself, built and drawn as one. Never fetched or held; only its address
//! exists, and an empty one is never made. The ground tile under it, and its
//! place in that tile, derive from its column, so they cannot disagree with
//! where it is.

use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::space::hierarchy::{ancestor_of, child_of, offset_in, parent_of};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pyramid::{chunk_zoom, tile_at, tile_rows, Tile, CHUNK_STEPS, LEAF_Z, TILE_CELLS, TOP_Z};
use pxl_spec::space::v3::Aabb;
use pxl_spec::space::zoom::wrap_cell;
use pxl_spec::space::zoom::{base_of, index_of, units, Zoom};

pub const CHILDREN: usize = 8;

/// The coarsest chunk: the top tile's.
pub const TOP_CHUNK: Zoom = chunk_zoom(TOP_Z);

/// The leaves' chunk: the finest whose cells fold a tile's.
pub const LEAF_CHUNK: Zoom = chunk_zoom(LEAF_Z);

/// The finest chunk: its cells are the objects' finest.
pub const FINEST_CHUNK: Zoom = OBJECT_FINEST - CHUNK_STEPS;

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

    /// Its cells': the camera grid's, `CHUNK_STEPS` zooms finer.
    pub fn cell_zoom(self) -> Zoom {
        self.zoom + CHUNK_STEPS
    }

    /// The ground tile under it: the one whose cells fold into its own, or past the leaves the leaf.
    pub fn tile_zoom(self) -> Zoom {
        (self.zoom - (LEAF_CHUNK - LEAF_Z)).min(LEAF_Z)
    }

    /// Its side in its tile's cells.
    fn tile_cells(self) -> u32 {
        TILE_CELLS >> (self.zoom - self.tile_zoom())
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

    /// Its first cell's place in its tile, in the tile's cells.
    pub fn in_tile(self) -> (u32, u32) {
        let [x, y] = offset_in(self.column().1, u32::from(self.zoom - self.tile_zoom()));
        (x as u32 * self.tile_cells(), y as u32 * self.tile_cells())
    }

    /// Neighbouring tiles an apron of `apron` of the tile's cells reaches into, with
    /// their offsets from the chunk's tile; none for a chunk inside it.
    pub fn tiles_beside(self, apron: u32) -> impl Iterator<Item = ([i64; 2], Tile)> {
        let (x0, y0) = self.in_tile();
        let side = self.tile_cells();
        let steps = move |v: u32| [0, -1, 1].into_iter().filter(move |&q| q == 0 || (q < 0 && v < apron) || (q > 0 && v + side + apron > TILE_CELLS));
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
