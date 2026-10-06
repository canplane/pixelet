// pxl-spec/space/layer/demand.rs

//! What an observation requires, before anything is a tile: information at a
//! zoom over a region (`Requirement`), named as the tiles under it (`tiles`).
//! What estimates them names a superset of the exact demand from what is
//! known; never less, since a requirement left out is ground that never
//! refines. What is known of a box is asked
//! in the grid's cells (`Bounds`); a layer says which of its zooms folds
//! into them by its lattice.

use super::lattice::{Lattice, GROUND};
use super::tile::{tile_at, tile_rows, Tile, LEAF_ZOOM};
use crate::space::camera::eye::Observation;
use crate::space::camera::grid::{split_distance, Tau};
use crate::space::coord::v3::Aabb;
use crate::space::coord::world::REACH;
use crate::space::coord::zoom::{units, Zoom};

/// What stands in `region` known at `z`; the leaves' for anything finer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Requirement {
    pub zoom: Zoom,
    pub region: Aabb,
}

/// What a box holds at a zoom, as far as is known.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Contents {
    Unknown,
    Empty,
    /// Everything is inside `bound`. Conservative, not tight; how far what is
    /// drawn may lie off it is the cell the reader draws, alike on every axis.
    Holds {
        bound: Aabb,
    },
}

impl Contents {
    /// Where in `region` anything can stand.
    pub fn within(self, region: Aabb) -> Option<Aabb> {
        match self {
            Contents::Unknown => Some(region),
            Contents::Empty => None,
            Contents::Holds { bound, .. } => Some(bound),
        }
    }
}

/// What is known of the world, read for the grid's cells at `cell`. Never
/// less than the truth, and at a coarser cell never less than at a finer one
/// under it.
pub trait Bounds {
    fn contents(&self, cell: Zoom, region: Aabb) -> Contents;

    /// The finest cell zoom anything in `region` is held at.
    fn finest(&self, region: Aabb) -> Zoom;
}

pub struct Unknown;

impl Bounds for Unknown {
    fn contents(&self, _cell: Zoom, _region: Aabb) -> Contents {
        Contents::Unknown
    }

    /// The ground's cell, held everywhere.
    fn finest(&self, _region: Aabb) -> Zoom {
        GROUND.cell()
    }
}

/// Every tile under a requirement's region; edges are inclusive south and west only.
pub fn tiles(r: &Requirement, each: &mut dyn FnMut(Tile)) {
    let span = units(r.zoom);
    let rows = |k: usize| {
        let first = (r.region.min[k] / span).floor() as i64;
        first..=((r.region.max[k] / span).ceil() as i64 - 1).max(first)
    };
    let ys = tile_rows(r.zoom);
    for y in rows(1).filter(|y| ys.contains(y)) {
        for x in rows(0) {
            each(tile_at(r.zoom, [x, y]));
        }
    }
}

/// The distances a layer's `zoom` is required over: from where the grid's
/// cell its tiles fold into begins (0 for the leaves) to where the next
/// coarser one's does, or the reach of the source's `top` at `zoom` alone,
/// unscaled. None past the far edge.
pub fn band(o: &Observation, tau: Tau, lattice: Lattice, top: Zoom, zoom: Zoom) -> Option<(f64, f64)> {
    let from = if zoom == LEAF_ZOOM { 0.0 } else { tau.split_distance(lattice.grid_cell(zoom)) };
    let coarser = lattice.grid_cell(zoom - 1);
    let to = if zoom == top { split_distance(coarser) } else { tau.split_distance(coarser) };
    (from <= o.far()).then_some((from, to))
}

/// The most tiles a zoom's demand span along an axis: ⌊d / tile⌋ + 2, the same at every zoom.
pub const REACH_ACROSS: u32 = (2.0 * GROUND.demand_reach(LEAF_ZOOM) / units(LEAF_ZOOM)) as u32 + 2;

/// The most tiles a zoom's demand name about one observer.
pub const REACH_TILES: u32 = REACH_ACROSS * REACH_ACROSS;

/// Every tile at `z` with ground within `radius` of `at`.
pub fn tiles_within(zoom: Zoom, at: [f64; 2], radius: f64) -> impl Iterator<Item = Tile> {
    let ys = tile_rows(zoom);
    squares_within(zoom, at, radius).filter(move |&[_, y]| ys.contains(&y)).map(move |xy| tile_at(zoom, xy))
}

/// `tiles_within` before X wraps.
pub fn squares_within(zoom: Zoom, at: [f64; 2], radius: f64) -> impl Iterator<Item = [i64; 2]> {
    let span = units(zoom);
    let rows = |c: f64| ((c - radius) / span).floor() as i64..=((c + radius) / span).floor() as i64;
    let (xs, ys) = (rows(at[0]), rows(at[1]));
    let gap = move |c: f64, i: i64| (i as f64 * span - c).max(c - (i + 1) as f64 * span).max(0.0);
    ys.flat_map(move |y| xs.clone().map(move |x| [x, y]))
        .filter(move |&[x, y]| libm::hypot(gap(at[0], x), gap(at[1], y)) <= radius)
}

/// The cubes `side` wide meeting `b` inside `REACH`.
pub fn cubes_meeting(side: f64, b: Aabb) -> impl Iterator<Item = [i64; 3]> {
    let b = b.meet(REACH).map(|b| {
        let row = |k: usize| {
            let hi = if b.max[k] < REACH.max[k] { (b.max[k] / side).floor() } else { (b.max[k] / side).ceil() - 1.0 };
            (b.min[k] / side).floor() as i64..=hi as i64
        };
        [row(0), row(1), row(2)]
    });
    b.into_iter().flat_map(|[xs, ys, zs]| {
        zs.flat_map(move |z| {
            let xs = xs.clone();
            ys.clone().flat_map(move |y| xs.clone().map(move |x| [x, y, z]))
        })
    })
}

#[cfg(test)]
#[path = "demand.test.rs"]
mod test;
