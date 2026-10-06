// pxl-spec/space/layer/read.rs

//! What a layer reads for a chunk, named by its zoom and column: the tile
//! whose cells fold into the chunk's (`CHUNK_STEPS` finer), or past the
//! leaves its leaf, and where the chunk lies in it. The view's side of a
//! lattice; the lattice itself knows only its cells and tiles.

use super::lattice::{Lattice, GROUND, TILE_CELLS};
use super::tile::{tile_at, tile_rows, Tile, LEAF_ZOOM};
use crate::space::camera::grid::{CHUNK_CELLS, CHUNK_STEPS};
use crate::space::coord::pvf3::ivec;
use crate::space::coord::zoom::Zoom;
use crate::space::hierarchy::tree::{ancestor_of, offset_in};

impl Lattice {
    /// The zoom of the tile a chunk at `chunk_zoom` reads.
    pub const fn read_zoom(self, chunk_zoom: Zoom) -> Zoom {
        let z = self.grid_z(chunk_zoom + CHUNK_STEPS);
        (if z < LEAF_ZOOM as i32 { z } else { LEAF_ZOOM as i32 }) as Zoom
    }

    /// A chunk's side in the cells of the tile it reads.
    pub const fn read_side(self, chunk_zoom: Zoom) -> u32 {
        self.tile_cells() >> (chunk_zoom - self.read_zoom(chunk_zoom))
    }

    pub fn read_tile(self, (zoom, column): (Zoom, [i64; 2])) -> Tile {
        let z = self.read_zoom(zoom);
        tile_at(z, ancestor_of(column, u32::from(zoom - z)))
    }

    /// The chunk's first cell's place in the tile it reads, in that tile's cells.
    pub fn read_at(self, (zoom, column): (Zoom, [i64; 2])) -> (u32, u32) {
        let [x, y] = offset_in(column, u32::from(zoom - self.read_zoom(zoom)));
        let side = self.read_side(zoom);
        (x as u32 * side, y as u32 * side)
    }

    /// Neighbouring tiles an apron of `apron` of the tile's cells reaches
    /// into, with their offsets from the chunk's tile; none for a chunk inside it.
    pub fn read_beside(self, chunk: (Zoom, [i64; 2]), apron: u32) -> impl Iterator<Item = ([i64; 2], Tile)> {
        let (x0, y0) = self.read_at(chunk);
        let (side, across) = (self.read_side(chunk.0), self.tile_cells());
        let steps = move |v: u32| [0, -1, 1].into_iter().filter(move |&q| q == 0 || (q < 0 && v < apron) || (q > 0 && v + side + apron > across));
        let tile = self.read_tile(chunk);
        let [px, py] = [ivec::X(tile.tile), ivec::Y(tile.tile)];
        let zoom = tile.zoom;
        let rows = tile_rows(zoom);
        steps(y0)
            .flat_map(move |qy| steps(x0).map(move |qx| [qx, qy]))
            .filter(move |&[qx, qy]| [qx, qy] != [0, 0] && rows.contains(&(py + qy)))
            .map(move |[qx, qy]| ([qx, qy], tile_at(zoom, [px + qx, py + qy])))
    }
}

/// Chunks a tile's side holds at the zoom whose cells fold it.
pub const TILE_CHUNKS: u32 = TILE_CELLS / (CHUNK_CELLS << GROUND.fold());
