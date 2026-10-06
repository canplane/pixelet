// pxl-kernel/world/query/column.rs

//! What stands in a ground cell's column: the surface, bores, decks and piers, and what is placed there.

use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::bake::deck::Occupies;
use pxl_spec::bake::fold::majority;
use pxl_spec::cell::object::object_value_is_decor;
use pxl_spec::procedural::{pier_stands, Frame};
use pxl_spec::space::coord::adapter::epsg3857::u_per_m;
use pxl_spec::space::hierarchy::tree::child_of;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{Cell, LEAF_ZOOM};

use pxl_spec::space::coord::zoom::{base_of, count, units};
use xpute_core::math::scalar::round;

use super::ground::*;
use crate::layer::*;

#[derive(Default)]
pub(super) struct Lines {
    pub(super) boxes: Vec<[f64; 2]>,
    pub(super) air: Option<[f64; 2]>,
}

/// A leaf column as composed for both rays and footing, so ground, bores and
/// decks are combined in one place.
pub(super) struct Composed {
    pub(super) cell: [i64; 2],
    pub(super) top: Option<f64>,
    pub(super) lines: Lines,
}

/// Folds sub-cell fill to the cell by majority, a zoom at a time, with the
/// objects' tie rule (`majority`).
pub(super) fn filled_cell(filled: impl Fn([i64; 3]) -> bool) -> bool {
    const N: usize = count(OBJECT_FINEST, GROUND.cell()) as usize;
    let mut v = [false; N * N * N];
    for (k, f) in v.iter_mut().enumerate() {
        *f = filled([k % N, k / N % N, k / (N * N)].map(|c| c as i64));
    }
    let (mut m, mut cell_zoom) = (N, OBJECT_FINEST);
    while cell_zoom > GROUND.cell() {
        cell_zoom -= 1;
        let h = m / 2;
        for k in 0..h * h * h {
            let parent = [k % h, k / h % h, k / (h * h)].map(|c| c as i64);
            let eight: [bool; 8] = core::array::from_fn(|slot| {
                let [x, y, z] = child_of(parent, slot).map(|c| c as usize);
                v[(z * m + y) * m + x]
            });
            v[k] = majority(eight, cell_zoom);
        }
        m = h;
    }
    v[0]
}

impl<'a> Access<'a> {
    pub(super) fn composed(&self, cell: [i64; 2]) -> Answer<Composed> {
        let [ix, iy] = cell;
        let base = base_of(GROUND.cell(), [ix, iy, 0]);
        let ground = self.ground(GROUND.cell_of(LEAF_ZOOM, &base));
        ground.map(|g| Composed {
            cell,
            top: g.top(),
            lines: self.lines_over(GROUND.cell_of(LEAF_ZOOM, &base), g.height),
        })
    }

    /// Whether ground is there where it reaches: not bore air, and not taken by
    /// an object's voxel entry (air entries included).
    pub(super) fn holds(&self, c: &Composed, cz: i64) -> bool {
        let z = cz as f64 * units(GROUND.cell());
        !c.lines.air.is_some_and(|[lo, hi]| z >= lo && z < hi) && !self.taken(c.cell, cz)
    }

    /// Decks and bore air over a cell, as the chunk paints them (`Over::occupies`).
    pub(super) fn lines_over(&self, cell: Cell, ground_height: f64) -> Lines {
        let mut lines = Lines::default();
        let Some((src, body)) = self.terrain.drawn(cell.tile()) else { return lines };
        let (at, side) = (sample_in(cell, src), 1.0 / count(GROUND.cell_zoom(cell.tile_zoom()), GROUND.cell_zoom(src.zoom)) as f64);
        let cell_u = units(GROUND.cell());
        let ground = round(ground_height / cell_u) as i32;
        let height_u = |[lo, hi]: [i32; 2]| [f64::from(lo) * cell_u, f64::from(hi) * cell_u];
        let tile = Frame::of(src);
        let frame = Some(tile);
        let cell_per_m = u_per_m(tile.corner[1]) / cell_u;
        let [tx, ty] = at.map(|v| v as u32);
        let look = body.ground.cells[GROUND.cell_index(tx, ty) as usize];
        for o in body.decks.over_point(frame, at, side) {
            match o.occupies(ground, cell_per_m) {
                Occupies::Grade => {}
                Occupies::Above { slab, pier } => {
                    lines.boxes.push(height_u(slab));
                    if let Some(pier) = pier.and_then(|p| pier_stands(p, look, body.decks.over_point(frame, at, side), ground, cell_per_m)) {
                        lines.boxes.push(height_u(pier));
                    }
                }
                Occupies::Through { air } => lines.air = Some(height_u(air)),
            }
        }
        lines
    }

    /// Whether a voxel entry, air included, takes the cell.
    pub(super) fn taken(&self, [ix, iy]: [i64; 2], cz: i64) -> bool {
        let mut taken = false;
        self.objects.each_in(GROUND.cell(), [ix, iy, cz], &mut |e| taken |= !object_value_is_decor(e.state));
        taken
    }
}
