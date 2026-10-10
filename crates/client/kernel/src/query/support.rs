// pxl-kernel/query/support.rs

//! Where something stands: the support, floor and ceiling over a point, climbing as a walker would.

use super::block::{Run, Surface};
use super::world::World;
use pxl_spec::block::stand::Block;
use pxl_spec::space::coord::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::coord::world::REACH;
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

/// The top of what fills `z` and everything touching it above; `z` where nothing does.
fn open_over(runs: &[Run], z: f64) -> f64 {
    filled(runs).fold(z, |top, r| if r.lo <= top && top < r.hi { r.hi } else { top })
}

/// What is in the block of the column `runs` at height `z`; none where it is void.
pub(super) fn block_in(runs: &[Run], z: f64) -> Option<Block> {
    runs.iter().find(|r| r.lo <= z && z < r.hi).map(|r| r.block)
}

fn solid(runs: &[Run], z: f64) -> bool {
    filled(runs).any(|r| r.lo <= z && z < r.hi)
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

    /// From the center `at` of something standing, down to the first solid's
    /// top, or up at most `rise` cells over its floor; taller gives none.
    pub fn support(&self, at: &Pvf3, rise: i64) -> Answer<Option<f64>> {
        let z = pvf3::to_real(pvf3::Z(at));
        self.stand_climbing(at, Some(z), Some(rise as f64 * units(GROUND.cell())))
    }

    /// `support` from the drawn surface, ignoring `at`'s height (a fix, a search
    /// result); none where nothing stands, a column with no surface and nothing placed.
    pub fn surface_support(&self, at: &Pvf3) -> Answer<Option<f64>> {
        self.stand_climbing(at, None, None)
    }

    /// `support` rising any height; none where nothing stands under `at`.
    pub fn floor_under(&self, at: &Pvf3) -> Answer<Option<f64>> {
        self.stand_climbing(at, Some(pvf3::to_real(pvf3::Z(at))), None)
    }

    /// The bottom of the first solid over `at`; none under open sky.
    pub fn ceiling_over(&self, at: &Pvf3) -> Answer<Option<f64>> {
        let z = pvf3::to_real(pvf3::Z(at));
        self.runs_under(at, |runs, _| filled(runs).find(|r| r.lo > z).map(|r| r.lo))
    }

    /// The lowest open height at or over `at`.
    pub fn open_from(&self, at: &Pvf3) -> Answer<f64> {
        let z = pvf3::to_real(pvf3::Z(at));
        self.runs_under(at, |runs, _| open_over(runs, z))
    }

    fn stand_climbing(&self, at: &Pvf3, from: Option<f64>, rise: Option<f64>) -> Answer<Option<f64>> {
        self.runs_under(at, |runs, surface| {
            let [lo, hi] = [REACH.min[2], REACH.max[2]];
            // With no surface, the search starts at the highest solid, or at the reach's floor.
            let start = from.or(surface).or(filled(runs).last().map(|r| r.hi)).unwrap_or(lo).max(lo);
            if solid(runs, start) {
                let over = open_over(runs, start).min(hi);
                // Climbed from the floor under the center.
                return match rise {
                    None => Some(over),
                    Some(rise) => (over - (start - units(GROUND.cell()) / 2.0) <= rise).then_some(over),
                };
            }
            filled(runs).rev().find(|r| r.hi <= start && r.hi > lo).map(|r| r.hi).or(surface.map(|_| start))
        })
    }

    /// The runs and surface of the leaf column under `at`, as the chunks draw it.
    fn runs_under<R>(&self, at: &Pvf3, f: impl FnOnce(&[Run], Option<f64>) -> R) -> Answer<R> {
        self.runs_at(at, LEAF_ZOOM, |runs, surface| f(runs, surface.map(|s| s.top)))
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
