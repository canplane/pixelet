// pxl-kernel/query/support.rs

//! Where something stands: the support, floor and ceiling over a point, climbing as a walker would.

use super::block::{Run, Surface};
use super::ray::Detail;
use super::world::World;
use pxl_spec::block::stand::Block;
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::coord::world::{REACH, ROUND};
use xpute_kit::math::scalar::wrap;

use pxl_spec::space::layer::lattice::GROUND;

use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use crate::layer::*;

/// The center's height over a floor whose top is `top`: half a cell up, as a
/// cell's center stands over its ground.
pub fn over(top: f64) -> f64 {
    top + units(GROUND.cell()) / 2.0
}

fn filled(runs: &[Run]) -> impl DoubleEndedIterator<Item = &Run> {
    runs.iter().filter(|r| r.fills)
}

/// What is in the block of the column `runs` at height `z`; none where it is void.
pub(super) fn block_in(runs: &[Run], z: f64) -> Option<Block> {
    runs.iter().find(|r| r.lo <= z && z < r.hi).map(|r| r.block)
}

impl<'a> World<'a> {
    /// The ground's surface under `at`, as the tile zoom `zoom`'s chunks draw
    /// it; none where the column has no ground.
    pub fn surface(&self, at: &Pvf3, zoom: Zoom) -> Answer<Option<Surface>> {
        self.runs_at(at, zoom, |_, surface| surface)
    }

    /// What is in the block at `at`, as the chunks draw it; none where it is void.
    pub fn block_of(&self, at: &Pvf3) -> Answer<Option<Block>> {
        self.block_at(at, LEAF_ZOOM)
    }

    /// As `block_of`, as the tile zoom `zoom`'s chunks draw it.
    pub fn block_at(&self, at: &Pvf3, zoom: Zoom) -> Answer<Option<Block>> {
        let z = pvf3::to_real(pvf3::Z(at));
        self.runs_at(at, zoom, |runs, _| block_in(runs, z))
    }

    /// Where a walker stands from the drawn surface, across the whole ground
    /// cell, ignoring `at`'s height (a fix, a search
    /// result); none where nothing stands, a column with no surface and nothing placed.
    pub fn surface_support(&self, at: &Pvf3) -> Answer<Option<f64>> {
        self.stand_climbing(at)
    }

    /// From the center `at` down to the first solid's top, or up out of what
    /// fills it, across the whole ground cell as a walker is; none where
    /// nothing stands under `at`.
    pub fn floor_under(&self, at: &Pvf3) -> Answer<Option<f64>> {
        let start = pvf3::to_real(pvf3::Z(at));
        self.cell_under(at, LEAF_ZOOM, |runs| {
            let mut z = start;
            while let Some(top) = filled(runs).filter(|r| r.lo <= z && z < r.hi).map(|r| r.hi).reduce(f64::max) {
                z = top;
            }
            if z > start {
                return Some(z.min(REACH.max[2]));
            }
            filled(runs).filter(|r| r.hi <= z && r.hi > REACH.min[2]).map(|r| r.hi).reduce(f64::max)
        })
    }

    /// The bottom of the first solid over `at`, across the whole ground cell;
    /// none under open sky.
    pub fn ceiling_over(&self, at: &Pvf3) -> Answer<Option<f64>> {
        let z = pvf3::to_real(pvf3::Z(at));
        self.cell_under(at, LEAF_ZOOM, |runs| filled(runs).filter(|r| r.lo > z).map(|r| r.lo).reduce(f64::min))
    }

    /// Where a walker standing at the center `from` ends going toward `to`,
    /// a column at a time, each column the ground cell the camera grid draws
    /// at its distance from `eye`, at the criterion's own τ so that the
    /// quality a device draws at moves no wall; with `falls` off it keeps its height wherever
    /// nothing lifts it, as the orbit's eye does. Quake III's PM_StepSlideMove
    /// (id Software, code/game/bg_slidemove.c): blocked, it tries again a step
    /// higher, gives up where the step up is blocked overhead, and settles on
    /// the floor; the step here has no height but the ceiling's, and an axis
    /// that is blocked is slid along, as the velocity is clipped to the wall.
    /// Where nothing is held of a column on the way it stops there; none where
    /// nothing is held under `from`.
    pub fn step_slide(&self, from: &Pvf3, to: &Pvf3, eye: &Pvf3, falls: bool) -> Option<Pvf3> {
        let detail = Detail::Distance(Tau::FINEST);
        let real = |p: &Pvf3| [pvf3::to_real(pvf3::X(p)), pvf3::to_real(pvf3::Y(p)), pvf3::to_real(pvf3::Z(p))];
        let ([sx, sy, sz], eye) = (real(from), real(eye));
        // The short way round the X lane, across the antimeridian too.
        let way = [wrap(pvf3::to_real(pvf3::X(to)) - sx, -ROUND / 2.0, ROUND / 2.0), pvf3::to_real(pvf3::Y(to)) - sy];
        let mut floor = sz - units(GROUND.cell()) / 2.0;
        // The column's tile zoom and its cell's side where the walker is at `p`.
        let zoom_at = |p: [f64; 2], floor: f64| {
            let d = [wrap(p[0] - eye[0], -ROUND / 2.0, ROUND / 2.0), p[1] - eye[1], over(floor) - eye[2]];
            detail.at(libm::sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2])).0.min(LEAF_ZOOM)
        };
        let side = |zoom: Zoom| units(GROUND.cell_zoom(zoom));
        let column = |c: [f64; 2], zoom: Zoom| {
            let s = side(zoom);
            pvf3::gather(pvf3::from_real((c[0] + 0.5) * s), pvf3::from_real((c[1] + 0.5) * s), 0)
        };
        let mut end = [sx + way[0], sy + way[1]];
        let mut blocked = [false; 2];
        // Where the walker is at the share `t` of the way, a slid axis held where it stopped.
        let point = |t: f64, end: [f64; 2], blocked: [bool; 2]| [0, 1].map(|k| if blocked[k] { end[k] } else { [sx, sy][k] + way[k] * t });
        let mut zoom = zoom_at([sx, sy], floor);
        let mut at = [sx, sy].map(|v| libm::floor(v / side(zoom)));
        // Out of what fills its own column at that zoom first, as `floor_under` is:
        // a coarse cell over the leaf floor is no ceiling.
        let (lifted, mut ceiling) = self
            .cell_under(&column(at, zoom), zoom, |runs| {
                let mut z = floor;
                while let Some(top) = filled(runs).filter(|r| r.lo <= z && z < r.hi).map(|r| r.hi).reduce(f64::max) {
                    z = top;
                }
                (z, filled(runs).filter(|r| r.lo > z).map(|r| r.lo).reduce(f64::min))
            })
            .value()?;
        floor = lifted;
        // Amanatides and Woo's traversal, its cell the column's own: the share of `way` at each axis's next edge.
        loop {
            let s = side(zoom);
            let edge = |k: usize| if way[k] > 0.0 { (at[k] + 1.0) * s } else { at[k] * s };
            let share = |k: usize| if blocked[k] || way[k] == 0.0 { f64::INFINITY } else { (edge(k) - [sx, sy][k]) / way[k] };
            let k = if share(0) <= share(1) { 0 } else { 1 };
            let t = share(k);
            if t > 1.0 {
                break;
            }
            // Just past the edge, in the column across it, at that column's zoom.
            // The other axis held inside this column, where a corner is met on both at once.
            let mut past = point(t, end, blocked);
            let j = 1 - k;
            past[j] = past[j].clamp(at[j] * s, (at[j] + 1.0) * s - s * 1e-6);
            past[k] = edge(k) + way[k].signum() * s * 1e-6;
            let into_zoom = zoom_at(past, floor);
            let into = past.map(|v| libm::floor(v / side(into_zoom)));
            // Short of the edge by less than PVF3's fraction, so the cell it ends in is this one.
            let short = edge(k) - way[k].signum() * s * 1e-6;
            match self.step(ceiling, floor, &column(into, into_zoom), into_zoom, falls) {
                Some(Some((top, over))) => (at, zoom, floor, ceiling) = (into, into_zoom, top, over),
                // Slid along: this axis stays in its column, the other goes on.
                Some(None) => (end[k], blocked[k]) = (short, true),
                None => {
                    end = point(t, end, blocked);
                    end[k] = short;
                    break;
                }
            }
        }
        Some(pvf3::gather(pvf3::from_real(end[0]), pvf3::from_real(end[1]), pvf3::from_real(over(floor))))
    }

    /// Into the column `to` at the tile zoom `zoom`, the feet at `floor` under
    /// the column left's `ceiling`: up over what fills the walker's cell, a
    /// cell of that zoom tall, while under it, then down onto the floor if it
    /// `falls`; the floor and the ceiling over it there, none where the
    /// ceiling stops it, the outer none where nothing of `to` is held.
    fn step(&self, ceiling: Option<f64>, floor: f64, to: &Pvf3, zoom: Zoom, falls: bool) -> Option<Option<(f64, Option<f64>)>> {
        let cell = units(GROUND.cell_zoom(zoom));
        let answer = self.cell_under(to, zoom, |runs| {
            let mut z = floor;
            while let Some(top) = filled(runs).filter(|r| r.lo < z + cell && r.hi > z).map(|r| r.hi).reduce(f64::max) {
                z = top;
            }
            if z > floor && ceiling.is_some_and(|c| c < z + cell) {
                return None;
            }
            if z == floor && falls {
                z = filled(runs).filter(|r| r.hi <= z).map(|r| r.hi).reduce(f64::max).unwrap_or(z);
            }
            Some((z, filled(runs).filter(|r| r.hi > z).map(|r| r.lo).reduce(f64::min)))
        });
        answer.value()
    }

    /// Every sub-column of the ground cell under `at` at the tile zoom `zoom`:
    /// a walker is a ground cell wide, so what is held finer anywhere in the
    /// cell fills it.
    fn cell_under<R>(&self, at: &Pvf3, zoom: Zoom, f: impl FnOnce(&[Run]) -> R) -> Answer<R> {
        self.cell(GROUND.cell_of(zoom, at).node().1, zoom, |c| f(c.all()))
    }

    fn stand_climbing(&self, at: &Pvf3) -> Answer<Option<f64>> {
        self.cell(GROUND.cell_of(LEAF_ZOOM, at).node().1, LEAF_ZOOM, |c| {
            let (runs, surface) = (c.all(), c.surface_top());
            let [lo, hi] = [REACH.min[2], REACH.max[2]];
            // With no surface, the search starts at the highest solid, or at the reach's floor.
            let start = surface.or(filled(runs).map(|r| r.hi).reduce(f64::max)).unwrap_or(lo).max(lo);
            let mut z = start;
            while let Some(top) = filled(runs).filter(|r| r.lo <= z && z < r.hi).map(|r| r.hi).reduce(f64::max) {
                z = top;
            }
            if z > start {
                return Some(z.min(hi));
            }
            filled(runs).filter(|r| r.hi <= start && r.hi > lo).map(|r| r.hi).reduce(f64::max).or(surface.map(|_| start))
        })
    }

    /// As `runs_under`, the column of the tile zoom `zoom`'s cell.
    pub(super) fn runs_at<R>(&self, at: &Pvf3, zoom: Zoom, f: impl FnOnce(&[Run], Option<Surface>) -> R) -> Answer<R> {
        let cell = GROUND.cell_of(zoom, at);
        let xy = [pvf3::to_real(pvf3::X(at)), pvf3::to_real(pvf3::Y(at))];
        self.cell(cell.node().1, zoom, |c| {
            let (runs, surface) = c.under(xy);
            f(runs, surface)
        })
    }
}
