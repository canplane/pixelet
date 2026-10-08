// pxl-kernel/query/ground.rs

//! What the world reads of the ground: the drawn surface and the index lane's bounds.

use super::world::World;
use crate::layer::terrain::Drawn;
use pxl_render::mesh::paint::terrain::elevation::ground_ele_at;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{Cell, Tile};

use pxl_spec::space::coord::zoom::{count, units};

use crate::layer::*;

/// Where `cell`'s center falls in the tile `src` it is drawn from, in that tile's cells.
pub(super) fn sample_in(cell: Cell, src: Tile) -> [f64; 2] {
    let n = count(GROUND.cell_zoom(cell.tile_zoom()), GROUND.cell_zoom(src.zoom)) as f64;
    cell.in_tile_above(cell.tile_zoom() - src.zoom).map(|o| (o as f64 + 0.5) / n)
}

impl<'a> World<'a> {
    /// Height at the cell's center in the drawn-from tile, sampled as
    /// `ChunkSample::read` samples a chunk drawn from a coarser tile.
    pub(super) fn ground(&self, cell: Cell) -> Answer<Ground> {
        let tile = cell.tile();
        match self.terrain.drawn(tile) {
            Some(d) => self.ground_in(cell, &d),
            None => Answer::Nothing(tile),
        }
    }

    /// As `ground`, the cell's tile drawn as `d` says.
    pub(super) fn ground_in(&self, cell: Cell, d: &Drawn) -> Answer<Ground> {
        let (src, body) = (d.from, d.body);
        let [tx, ty] = sample_in(cell, src);
        let cell = body.ground.cells[GROUND.cell_index(tx as u32, ty as u32) as usize];
        let height = ground_ele_at(&body.ele, tx, ty) as f64 * units(GROUND.cell());
        d.answer(Ground { height, cell })
    }
}
