// pxl-kernel/query/column.rs

//! What stands in a ground cell's column: the surface, bores, decks and piers, and what is placed there.

use super::world::World;
use crate::layer::terrain::Drawn;
use crate::layer::Query;
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::bake::deck::Occupies;
use pxl_spec::bake::fold::majority;
use pxl_spec::cell::object::object_value_is_decor;
use pxl_spec::procedural::{pier_stands, Frame};
use pxl_spec::space::coord::adapter::epsg3857::u_per_m;
use pxl_spec::space::hierarchy::tree::child_of;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{Cell, LEAF_ZOOM};

use pxl_spec::space::coord::zoom::{base_of, count, units, Zoom};
use xpute_core::math::scalar::round;

use super::ground::*;
use crate::layer::*;

/// Decks and a bore over a cell, in whole leaf cells, each `lo..hi`.
#[derive(Default)]
pub(super) struct Lines {
    pub(super) boxes: Vec<[i64; 2]>,
    pub(super) air: Option<[i64; 2]>,
}

/// A column as the chunk paints its ground, at any zoom: its cell's
/// footprint, and heights in whole leaf cells, for rays and footing alike.
pub(super) struct Composed {
    /// X and Y at `zoom`.
    pub(super) cell: [i64; 2],
    pub(super) zoom: Zoom,
    /// The first cell over the surface; none where the column has none.
    pub(super) top: Option<i64>,
    pub(super) lines: Lines,
}

impl Composed {
    /// The ground fills `cz`: under the surface and out of a bore's air, or
    /// in a deck's slab or pier.
    pub(super) fn ground_at(&self, cz: i64) -> bool {
        let inside = |[lo, hi]: [i64; 2]| (lo..hi).contains(&cz);
        (self.top.is_some_and(|t| cz < t) && !self.lines.air.is_some_and(inside)) || self.lines.boxes.iter().copied().any(inside)
    }

    /// The first cell over all the ground fills.
    pub(super) fn ceiling(&self) -> Option<i64> {
        self.top.into_iter().chain(self.lines.boxes.iter().map(|b| b[1])).max()
    }
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

impl<'a> World<'a> {
    pub(super) fn composed(&self, cell: [i64; 2]) -> Answer<Composed> {
        let leaf = GROUND.cell_of(LEAF_ZOOM, &base_of(GROUND.cell(), [cell[0], cell[1], 0]));
        match self.terrain.drawn(leaf.tile()) {
            Some(d) => self.composed_in(cell, LEAF_ZOOM, &d),
            None => Answer::Nothing(leaf.tile()),
        }
    }

    /// The column of `cell` at `zoom`, its tile drawn as `d` says.
    pub(super) fn composed_in(&self, cell: [i64; 2], zoom: Zoom, d: &Drawn) -> Answer<Composed> {
        let at = GROUND.cell_of(zoom, &base_of(GROUND.cell_zoom(zoom), [cell[0], cell[1], 0]));
        let cell_u = units(GROUND.cell());
        self.ground_in(at, d).map(|g| Composed {
            cell,
            zoom,
            top: g.top().map(|t| round(t / cell_u) as i64),
            lines: self.lines_in(at, d, g.height),
        })
    }

    /// The ground fills `cz` and, in a leaf column, no object's voxel entry,
    /// air included, takes it: a folded cell is a summary nothing cuts.
    pub(super) fn holds(&self, c: &Composed, cz: i64) -> bool {
        c.ground_at(cz) && (c.zoom < LEAF_ZOOM || !self.taken(c.cell, cz))
    }

    /// Decks and bore air over a cell, as the chunk paints them
    /// (`Over::occupies`), its tile drawn as `d` says.
    pub(super) fn lines_in(&self, cell: Cell, d: &Drawn, ground_height: f64) -> Lines {
        let mut lines = Lines::default();
        let (src, body) = (d.from, d.body);
        let (at, side) = (sample_in(cell, src), 1.0 / count(GROUND.cell_zoom(cell.tile_zoom()), GROUND.cell_zoom(src.zoom)) as f64);
        let cell_u = units(GROUND.cell());
        let ground = round(ground_height / cell_u) as i32;
        let cells = |[lo, hi]: [i32; 2]| [i64::from(lo), i64::from(hi)];
        let tile = Frame::of(src);
        let frame = Some(tile);
        let cell_per_m = u_per_m(tile.corner[1]) / cell_u;
        let [tx, ty] = at.map(|v| v as u32);
        let look = body.ground.cells[GROUND.cell_index(tx, ty) as usize];
        for o in body.decks.over_point(frame, at, side) {
            match o.occupies(ground, cell_per_m) {
                Occupies::Grade => {}
                Occupies::Above { slab, pier } => {
                    lines.boxes.push(cells(slab));
                    if let Some(pier) = pier.and_then(|p| pier_stands(p, look, body.decks.over_point(frame, at, side), ground, cell_per_m)) {
                        lines.boxes.push(cells(pier));
                    }
                }
                Occupies::Through { air } => lines.air = Some(cells(air)),
            }
        }
        lines
    }

    /// Whether a voxel entry, air included, takes the cell.
    pub(super) fn taken(&self, [ix, iy]: [i64; 2], cz: i64) -> bool {
        let mut taken = false;
        if let Some(c) = self.objects.read((GROUND.cell(), [ix, iy, cz]), GROUND.cell()).value() {
            c.entries(&mut |e| taken |= !object_value_is_decor(e.state));
        }
        taken
    }
}
