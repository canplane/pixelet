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

/// The floor under a center at `z`: what `over` lifts it from.
pub fn under(z: f64) -> f64 {
    z - units(GROUND.cell()) / 2.0
}

/// The tile zoom the camera grid at `tau` draws the ground at, at `at` seen
/// from `eye`.
fn seen_zoom(at: &Pvf3, eye: &Pvf3, tau: Tau) -> Zoom {
    let [a, e] = [at, eye].map(|p| [pvf3::X(p), pvf3::Y(p), pvf3::Z(p)].map(pvf3::to_real));
    let d = [wrap(a[0] - e[0], -ROUND / 2.0, ROUND / 2.0), a[1] - e[1], a[2] - e[2]];
    Detail::Distance(tau).at(libm::sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2])).0.min(LEAF_ZOOM)
}

fn filled(runs: &[Run]) -> impl DoubleEndedIterator<Item = &Run> {
    runs.iter().filter(|r| r.fills)
}

/// Up out of what fills `runs` at `z`: the top of the stack it is inside, or `z`.
fn lift(runs: &[Run], mut z: f64) -> f64 {
    while let Some(top) = filled(runs).filter(|r| r.lo <= z && z < r.hi).map(|r| r.hi).reduce(f64::max) {
        z = top;
    }
    z
}

/// The top of the highest solid at or under `z`, over the reach's floor.
fn below(runs: &[Run], z: f64) -> Option<f64> {
    filled(runs).filter(|r| r.hi <= z && r.hi > REACH.min[2]).map(|r| r.hi).reduce(f64::max)
}

/// The bottom of the lowest solid over `z`.
fn above(runs: &[Run], z: f64) -> Option<f64> {
    filled(runs).filter(|r| r.lo > z).map(|r| r.lo).reduce(f64::min)
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
    /// fills it, across the whole ground cell of the tile zoom `zoom` as a
    /// walker is; none where nothing stands under `at`.
    pub fn floor_under(&self, at: &Pvf3, zoom: Zoom) -> Answer<Option<f64>> {
        let start = pvf3::to_real(pvf3::Z(at));
        self.cell_under(at, zoom, |runs| match lift(runs, start) {
            z if z > start => Some(z.min(REACH.max[2])),
            z => below(runs, z),
        })
    }

    /// The floor in the leaf column under `at` nearest its height, across the
    /// whole ground cell as a walker is: the top of a solid with a cell clear
    /// over it, or the column's surface; none where nothing stands.
    pub fn support_nearest(&self, at: &Pvf3) -> Answer<Option<f64>> {
        let (z, cell) = (pvf3::to_real(pvf3::Z(at)), units(GROUND.cell()));
        self.cell(GROUND.cell_of(LEAF_ZOOM, at).node().1, LEAF_ZOOM, |c| {
            let runs = c.all();
            let open = |top: f64| !filled(runs).any(|r| r.lo < top + cell && r.hi > top);
            filled(runs)
                .map(|r| r.hi)
                .filter(|&top| open(top))
                .chain(c.surface_top())
                .min_by(|a, b| (a - z).abs().total_cmp(&(b - z).abs()))
        })
    }

    /// The bottom of the first solid over `at`, across the whole ground cell;
    /// none under open sky.
    pub fn ceiling_over(&self, at: &Pvf3) -> Answer<Option<f64>> {
        let z = pvf3::to_real(pvf3::Z(at));
        self.cell_under(at, LEAF_ZOOM, |runs| above(runs, z))
    }

    /// Where a walker standing at the center `from` ends going toward `to`,
    /// a leaf column at a time. Our rule after Quake III's PM_StepSlideMove
    /// (id Software, code/game/bg_slidemove.c), which steps up and slides a
    /// blocked axis along the wall as PM_ClipVelocity clips the velocity:
    /// here the step up has no height but the ceiling's, since nothing jumps,
    /// it gives up where its own column's ceiling stops the climb, and it
    /// settles on the floor where it `falls`; with `falls` off it keeps its
    /// height wherever nothing lifts it, as the orbit's eye does. Where the
    /// camera grid at the criterion's own τ draws `from` coarser than the
    /// ground's own cells, seen from `eye`, this is a map, not a walk: it goes
    /// to `to` and stands there. That is decided where it starts, and by the
    /// criterion's τ so that the quality a device draws at moves no wall.
    /// It stops at the edge of a leaf on the way not held, keeping its walk up
    /// to there; none where the leaf under `from`, or a map's `to`, is not
    /// held, when its caller goes on with the height unknown. `from` is a
    /// center whose height is known.
    pub fn step_slide(&self, from: &Pvf3, to: &Pvf3, eye: &Pvf3, falls: bool) -> Option<Pvf3> {
        let [sx, sy, sz] = [pvf3::X(from), pvf3::Y(from), pvf3::Z(from)].map(pvf3::to_real);
        let mut floor = under(sz);
        let gather = |x: f64, y: f64, floor: f64| pvf3::gather(pvf3::from_real(x), pvf3::from_real(y), pvf3::from_real(over(floor)));
        let [tx, ty] = [pvf3::X(to), pvf3::Y(to)].map(pvf3::to_real);
        if seen_zoom(from, eye, Tau::FINEST) < LEAF_ZOOM {
            let at = pvf3::gather(pvf3::X(to), pvf3::Y(to), pvf3::Z(from));
            let stands = if falls {
                self.surface_support(&at).exact()?.unwrap_or(floor)
            } else {
                self.floor_under(&at, LEAF_ZOOM).exact()?.filter(|&z| z > floor).unwrap_or(floor)
            };
            return Some(gather(tx, ty, stands));
        }
        // The short way round the X lane, across the antimeridian too.
        let way = [wrap(tx - sx, -ROUND / 2.0, ROUND / 2.0), ty - sy];
        let s = units(GROUND.cell());
        let column = |c: [f64; 2]| pvf3::gather(pvf3::from_real((c[0] + 0.5) * s), pvf3::from_real((c[1] + 0.5) * s), 0);
        let mut end = [sx + way[0], sy + way[1]];
        let mut blocked = [false; 2];
        // Where the walker is at the share `t` of the way, a slid axis held where it stopped.
        let point = |t: f64, end: [f64; 2], blocked: [bool; 2]| [0, 1].map(|k| if blocked[k] { end[k] } else { [sx, sy][k] + way[k] * t });
        let mut at = [sx, sy].map(|v| libm::floor(v / s));
        // Out of what fills its own column first: only what is over that is a ceiling.
        let mut ceiling;
        (floor, ceiling) = self.cell_under(&column(at), LEAF_ZOOM, |runs| (lift(runs, floor), above(runs, lift(runs, floor)))).exact()?;
        // Amanatides and Woo's traversal: the share of `way` at each axis's next edge.
        loop {
            let edge = |k: usize| if way[k] > 0.0 { (at[k] + 1.0) * s } else { at[k] * s };
            let share = |k: usize| if blocked[k] || way[k] == 0.0 { f64::INFINITY } else { (edge(k) - [sx, sy][k]) / way[k] };
            let k = if share(0) <= share(1) { 0 } else { 1 };
            let t = share(k);
            if t > 1.0 {
                break;
            }
            let mut into = at;
            into[k] += way[k].signum();
            // Short of the edge by less than PVF3's fraction, so the cell it ends in is this one.
            let short = edge(k) - way[k].signum() * s * 1e-6;
            match self.step(ceiling, floor, &column(into), falls) {
                Some(Some((top, over))) => (at, floor, ceiling) = (into, top, over),
                // Slid along: this axis stays in its column, the other goes on.
                Some(None) => (end[k], blocked[k]) = (short, true),
                None => {
                    end = point(t, end, blocked);
                    end[k] = short;
                    break;
                }
            }
        }
        Some(gather(end[0], end[1], floor))
    }

    /// Into the leaf column `to`, the feet at `floor` under the column left's
    /// `ceiling`: up over what fills the walker's cell, a cell tall, while
    /// under it, then down onto the floor if it `falls`; the floor and the
    /// ceiling over it there, none where the ceiling stops it, the outer none
    /// where `to` is not held at the leaf.
    fn step(&self, ceiling: Option<f64>, floor: f64, to: &Pvf3, falls: bool) -> Option<Option<(f64, Option<f64>)>> {
        let cell = units(GROUND.cell());
        let answer = self.cell_under(to, LEAF_ZOOM, |runs| {
            let mut z = floor;
            while let Some(top) = filled(runs).filter(|r| r.lo < z + cell && r.hi > z).map(|r| r.hi).reduce(f64::max) {
                z = top;
            }
            if z > floor && ceiling.is_some_and(|c| c < z + cell) {
                return None;
            }
            if z == floor && falls {
                z = below(runs, z).unwrap_or(z);
            }
            Some((z, above(runs, z)))
        });
        answer.exact()
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
            match lift(runs, start) {
                z if z > start => Some(z.min(hi)),
                _ => below(runs, start).or(surface.map(|_| start)),
            }
        })
    }

    /// The column under `at` in the tile zoom `zoom`'s cell, and its surface.
    pub(super) fn runs_at<R>(&self, at: &Pvf3, zoom: Zoom, f: impl FnOnce(&[Run], Option<Surface>) -> R) -> Answer<R> {
        let cell = GROUND.cell_of(zoom, at);
        let xy = [pvf3::to_real(pvf3::X(at)), pvf3::to_real(pvf3::Y(at))];
        self.cell(cell.node().1, zoom, |c| {
            let (runs, surface) = c.under(xy);
            f(runs, surface)
        })
    }
}
