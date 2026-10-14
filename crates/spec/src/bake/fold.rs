// pxl-spec/bake/fold.rs

//! One zoom coarser: a parent tile from the four under it. A cell is
//! pxl-portable's `ground::fold` of the four under it — the looks' majority
//! (as Earth Engine's `MODE` pyramiding), so a feature narrower than half a
//! cell rounds away, and the grounded cells' mean height, rounded — the one
//! rule the client folds a grid cell by, so every zoom's cells nest: a
//! cell's height lies within the lowest and highest of the cells under it.

use crate::space::layer::lattice::Cells;
use crate::space::layer::lattice::{GROUND, TILE_CELLS};

use super::separation::fold_separation;
use super::tile::terrain::{Ground, TerrainTile};
use super::tile::Occupancy;
use crate::space::coord::zoom::Zoom;
use pxl_portable::ground::fold;

/// A parent at `zoom` from its four children, in `slot_of`'s order; None if
/// any is missing.
pub fn fold_terrain(children: [Option<&TerrainTile>; 4], zoom: Zoom) -> Option<TerrainTile> {
    let children = [children[0]?, children[1]?, children[2]?, children[3]?];
    let cell = GROUND.cell_zoom(zoom);
    let (cells, heights): (Vec<_>, Vec<_>) = (0..TILE_CELLS * TILE_CELLS)
        .map(|index| {
            let (q, block) = GROUND.child_cells(index);
            let c = children[q];
            let (look, h, _) = fold(block.map(|i| (u32::from(c.ground.cells[i as usize]), i32::from(c.heights[i as usize]))), cell);
            (look as u8, libm::rintf(h) as i16)
        })
        .unzip();
    Some(TerrainTile::of(
        heights,
        Ground { cells: cells.into() },
        fold_separation(children.map(|t| &t.separation), zoom),
        Occupancy::of_children(children.map(|t| t.occupancy)),
    ))
}

#[cfg(test)]
#[path = "fold.test.rs"]
mod test;
