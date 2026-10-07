// pxl-kernel/world/query/ground.rs

//! The query's handle, and what it reads of the ground: the drawn surface and the index lane's bounds.

use crate::layer::terrain::Drawn;
use pxl_render::mesh::paint::terrain::elevation::{ground_ele_at, GroundLanes};
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{Cell, Tile};

use crate::kernel;
use crate::layer::object::store::Store;
use crate::layer::residency::Residency;
use crate::layer::Layer as _;
use pxl_spec::space::coord::zoom::{count, units};

use crate::layer::*;

#[derive(Clone, Copy)]
pub struct Access<'a> {
    pub(super) terrain: &'a Residency<Terrain>,
    pub(super) objects: &'a Store,
    pub(super) visits: Option<&'a std::cell::Cell<u64>>,
}

impl Access<'static> {
    pub fn now() -> Access<'static> {
        let objects: &'static Store = &kernel().objects;
        Access::new(Terrain::residency(), objects)
    }
}

/// Where `cell`'s center falls in the tile `src` it is drawn from, in that tile's cells.
pub(super) fn sample_in(cell: Cell, src: Tile) -> [f64; 2] {
    let n = count(GROUND.cell_zoom(cell.tile_zoom()), GROUND.cell_zoom(src.zoom)) as f64;
    cell.in_tile_above(cell.tile_zoom() - src.zoom).map(|o| (o as f64 + 0.5) / n)
}

impl<'a> Access<'a> {
    pub fn new(terrain: &'a Residency<Terrain>, objects: &'a Store) -> Access<'a> {
        Access { terrain, objects, visits: None }
    }

    pub fn counting(self, visits: &'a std::cell::Cell<u64>) -> Access<'a> {
        Access { visits: Some(visits), ..self }
    }

    pub(super) fn visit(&self) {
        if let Some(v) = self.visits {
            v.set(v.get() + 1);
        }
    }

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
        let height = ground_ele_at(&body.ele, Some(GroundLanes::of(&body.ground)), tx, ty) as f64 * units(GROUND.cell());
        d.answer(Ground { height, cell })
    }
}
