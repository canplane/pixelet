// pxl-kernel/world/pyramid/read.rs

//! What a layer reads for a chunk: the tile of its lattice whose cells fold
//! into the chunk's, or past its leaves its leaf, and where the chunk lies
//! in it. The chunk names only the grid's cells; the layer's lattice says
//! which of its tiles holds them.

use pxl_spec::space::hierarchy::{ancestor_of, offset_in};
use pxl_spec::space::pvf3::ivec;
use pxl_spec::space::pyramid::{tile_at, tile_rows, Lattice, Tile, LEAF_Z};
use pxl_spec::space::zoom::Zoom;

use super::chunk::Chunk;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileRead {
    lattice: Lattice,
    chunk: Chunk,
}

impl TileRead {
    pub fn of(lattice: Lattice, chunk: Chunk) -> TileRead {
        TileRead { lattice, chunk }
    }

    pub fn chunk(self) -> Chunk {
        self.chunk
    }

    pub fn parent(self) -> Option<TileRead> {
        Some(TileRead { chunk: self.chunk.parent()?, ..self })
    }

    pub fn tile_zoom(self) -> Zoom {
        self.lattice.grid_z(self.chunk.cell_zoom()).min(i32::from(LEAF_Z)) as Zoom
    }

    /// The chunk's side in its tile's cells.
    pub fn tile_cells(self) -> u32 {
        self.lattice.tile_cells() >> (self.chunk.zoom() - self.tile_zoom())
    }

    pub fn tile(self) -> Tile {
        let (zoom, column) = self.chunk.column();
        tile_at(self.tile_zoom(), ancestor_of(column, u32::from(zoom - self.tile_zoom())))
    }

    /// The chunk's first cell's place in its tile, in the tile's cells.
    pub fn in_tile(self) -> (u32, u32) {
        let (zoom, column) = self.chunk.column();
        let [x, y] = offset_in(column, u32::from(zoom - self.tile_zoom()));
        (x as u32 * self.tile_cells(), y as u32 * self.tile_cells())
    }

    /// Neighbouring tiles an apron of `apron` of the tile's cells reaches into, with
    /// their offsets from the chunk's tile; none for a chunk inside it.
    pub fn tiles_beside(self, apron: u32) -> impl Iterator<Item = ([i64; 2], Tile)> {
        let (x0, y0) = self.in_tile();
        let (side, across) = (self.tile_cells(), self.lattice.tile_cells());
        let steps = move |v: u32| [0, -1, 1].into_iter().filter(move |&q| q == 0 || (q < 0 && v < apron) || (q > 0 && v + side + apron > across));
        let tile = self.tile();
        let [px, py] = [ivec::X(tile.tile), ivec::Y(tile.tile)];
        let zoom = self.tile_zoom();
        let rows = tile_rows(zoom);
        steps(y0)
            .flat_map(move |qy| steps(x0).map(move |qx| [qx, qy]))
            .filter(move |&[qx, qy]| [qx, qy] != [0, 0] && rows.contains(&(py + qy)))
            .map(move |[qx, qy]| ([qx, qy], tile_at(zoom, [px + qx, py + qy])))
    }
}
