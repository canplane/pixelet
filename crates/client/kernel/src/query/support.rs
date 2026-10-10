// pxl-kernel/query/support.rs

//! Where something stands: the support, floor and ceiling over a point, climbing as a walker would.

use super::world::World;
use crate::layer::object::store::VOID;
use crate::layer::Query;
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::cell::object::{object_value_is_air, object_value_is_voxel};
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::world::REACH;
use pxl_spec::space::layer::lattice::GROUND;

use pxl_spec::space::coord::zoom::{count, index_of, units, Zoom};

use super::column::*;
use crate::layer::*;

/// The center's height over a floor whose top is `top`: half a cell up, as a
/// cell's center stands over its ground.
pub fn over(top: f64) -> f64 {
    top + units(GROUND.cell()) / 2.0
}

impl<'a> World<'a> {
    pub fn at(&self, at: &Pvf3, zoom: Zoom) -> Answer<Ground> {
        self.ground(GROUND.cell_of(zoom, at))
    }

    /// Down to the first solid's top, or up at most `rise` cells; taller gives none.
    pub fn support(&self, at: &Pvf3, rise: i64) -> Answer<Option<f64>> {
        let [ix, iy, iz] = index_of(GROUND.cell(), at);
        self.stand_climbing([ix, iy], Some(iz), Some(rise))
    }

    /// `support` from the drawn surface, ignoring `at`'s height (a fix, a search
    /// result); none where nothing stands, a column with no surface and nothing placed.
    pub fn surface_support(&self, at: &Pvf3) -> Answer<Option<f64>> {
        let [ix, iy, _] = index_of(GROUND.cell(), at);
        self.stand_climbing([ix, iy], None, None)
    }

    /// `support` rising any height; none where nothing stands under `at`.
    pub fn floor_under(&self, at: &Pvf3) -> Answer<Option<f64>> {
        let [ix, iy, iz] = index_of(GROUND.cell(), at);
        self.stand_climbing([ix, iy], Some(iz), None)
    }

    /// First solid cell over `at`; none under open sky.
    pub fn ceiling_over(&self, at: &Pvf3) -> Answer<Option<i64>> {
        let [ix, iy, iz] = index_of(GROUND.cell(), at);
        self.cells([ix, iy], |_, ceiling, solid| (iz + 1..ceiling).find(|&cz| solid(cz)))
    }

    /// First open cell at or over `at`.
    pub fn open_from(&self, at: &Pvf3) -> Answer<i64> {
        let [ix, iy, iz] = index_of(GROUND.cell(), at);
        self.cells([ix, iy], |_, ceiling, solid| (iz..ceiling).find(|&cz| !solid(cz)).unwrap_or(iz.max(ceiling)))
    }

    pub(super) fn stand_climbing(&self, [ix, iy]: [i64; 2], from: Option<i64>, rise: Option<i64>) -> Answer<Option<f64>> {
        self.cells([ix, iy], |top, ceiling, solid| {
            let side = units(GROUND.cell());
            let [lo, hi] = [REACH.min[2] / side, REACH.max[2] / side].map(|z| z as i64);
            // With no surface, the search starts at the highest solid, or at the reach's floor.
            let start = from.or(top).unwrap_or(ceiling).max(lo);
            let climb = rise.unwrap_or(hi - start);
            let stand = if start < ceiling && solid(start) {
                let over = (start + 1..=start + climb).find(|&cz| !solid(cz));
                if rise.is_none() {
                    over.or(Some(start))
                } else {
                    over
                }
            } else {
                (lo..start.min(ceiling)).rev().find(|&cz| solid(cz)).map(|cz| cz + 1).or(top.map(|_| start))
            };
            stand.map(|cz| cz as f64 * side)
        })
    }

    /// Ground top, ceiling and solidity of a column, in cells; sub-cells fold by majority.
    pub(super) fn cells<R>(&self, [ix, iy]: [i64; 2], f: impl FnOnce(Option<i64>, i64, &dyn Fn(i64) -> bool) -> R) -> Answer<R> {
        self.composed([ix, iy]).map(|c| {
            let side = units(GROUND.cell());
            let n = i64::from(count(OBJECT_FINEST, GROUND.cell()));
            let solid = |cz: i64| {
                if !self.objects.read((GROUND.cell(), [ix, iy, cz]), GROUND.cell()).value().is_some_and(|c| c.holds()) {
                    return c.ground_at(cz);
                }
                filled_cell(|[x, y, z]| {
                    let node = (OBJECT_FINEST, [ix * n + x, iy * n + y, cz * n + z]);
                    let v = self.objects.read(node, OBJECT_FINEST).value().map_or(VOID, |c| c.node().0.state);
                    match object_value_is_voxel(v) {
                        true => !object_value_is_air(v),
                        false => c.ground_at(cz),
                    }
                })
            };
            let held = self.objects.bounds((GROUND.cell(), [ix, iy]), GROUND.cell()).value().flatten();
            let objects_top = held.map(|b| (b.aabb.max[2] / side).ceil() as i64);
            let ceiling = c.ceiling().max(objects_top).unwrap_or(i64::MIN);
            let top = c.top;
            f(top, ceiling, &solid)
        })
    }
}
