// pxl-spec/space/layer/demand.rs

//! What an observation requires: tiles, with their columns known at their zoom. What
//! estimates them names a superset of the exact demand from what is known, never less, since
//! a requirement left out is ground that never refines. What is known of a node is the core's
//! query (pxl-core domain/query.rs), asked in the grid's cells.

use super::lattice::{Lattice, GROUND};
use super::quadrant::{quad_rows, Quadrant, LEAF_ZOOM};
use crate::space::camera::eye::Observation;
use crate::space::camera::grid::split_dist;
use crate::space::coord::zoom::{units, Zoom};
use crate::space::layer::lattice::{demand_dist, draw_dist};

/// The distances over which a layer's `zoom` is required: from where the grid's cell its
/// tiles fold into begins (0 for the leaves) to where the next coarser one's does; or, for the
/// source's `top`, the distance at `zoom` alone, as far as a clip level holds it
/// (`demand_dist`). None past the far edge.
pub fn band(o: &Observation, lattice: Lattice, top: Zoom, zoom: Zoom) -> Option<(f64, f64)> {
    let from = if zoom == LEAF_ZOOM {
        0.0
    } else {
        split_dist(lattice.grid_zoom(zoom))
    };
    let to = if zoom == top {
        demand_dist(lattice, zoom)
    } else {
        draw_dist(lattice, zoom)
    };
    (from <= o.demand_dist).then_some((from, to))
}

/// The most of a lattice's tiles a zoom's demand spans along an axis: ⌊d / tile⌋ + 2, the same
/// at every zoom. A tile of fewer grid cells is asked for in greater numbers.
pub(crate) const fn demand_across(lattice: Lattice) -> u32 {
    (2.0 * demand_dist(lattice, LEAF_ZOOM) / units(LEAF_ZOOM)) as u32 + 2
}

/// The most of a lattice's tiles a zoom's demand names about one observer.
pub const fn demand_tiles(lattice: Lattice) -> u32 {
    demand_across(lattice) * demand_across(lattice)
}

/// The ground lattice's `demand_across`, to which the floor and the Earth's window are cut.
pub const GROUND_DEMAND_ACROSS: u32 = demand_across(GROUND);
pub const GROUND_DEMAND_TILES: u32 = demand_tiles(GROUND);

/// Every tile at `zoom` with ground within `radius` of `at`.
pub fn tiles_within(zoom: Zoom, at: [f64; 2], radius: f64) -> impl Iterator<Item = Quadrant> {
    let ys = quad_rows(zoom);
    squares_within(zoom, at, radius)
        .filter(move |&[_, y]| ys.contains(&y))
        .map(move |xy| Quadrant::new(zoom, xy))
}

/// `tiles_within` before X wraps.
pub fn squares_within(zoom: Zoom, at: [f64; 2], radius: f64) -> impl Iterator<Item = [i32; 2]> {
    let span = units(zoom);
    let rows = |c: f64| ((c - radius) / span).floor() as i32..=((c + radius) / span).floor() as i32;
    let (xs, ys) = (rows(at[0]), rows(at[1]));
    let gap = move |c: f64, i: i32| {
        (f64::from(i) * span - c)
            .max(c - f64::from(i + 1) * span)
            .max(0.0)
    };
    ys.flat_map(move |y| xs.clone().map(move |x| [x, y]))
        .filter(move |&[x, y]| libm::hypot(gap(at[0], x), gap(at[1], y)) <= radius)
}

#[cfg(test)]
#[path = "demand.test.rs"]
mod test;
