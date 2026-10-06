// pxl-spec/space/lod.rs

//! The cell ground is drawn at from its distance, and the tiles a zoom's
//! misses can name about a point. Both depend only on the lattice, so the
//! resident bound is proved before anything of the world or screen is known.

use super::pvf3::REACH;
use super::pyramid::{tile_at, tile_rows, Tile, DRAWN_STEPS, GROUND, LEAF_Z, TOP_Z};
use super::v3::Aabb;
use super::zoom::{units, Zoom, UNIT_Z};

/// The criterion as a distance per cell side: a zoom is drawn from `SPLIT`
/// of its cells away out to twice that. 192 hands the finest tier off at
/// 192u at 1080 px and 50°.
pub const SPLIT: f64 = 192.0;

/// The view `SPLIT` is stated at: 1080 px high and 50° across it.
pub const REFERENCE_VIEWPORT_H: f64 = 1080.0;
pub const REFERENCE_FOV_DEG: f64 = 50.0;

/// Pixels a unit at unit distance spans, for a viewport `viewport_h` high
/// and `fov_deg` across it.
pub fn px_per_unit(viewport_h: f64, fov_deg: f64) -> f64 {
    viewport_h / (2.0 * libm::tan((fov_deg * core::f64::consts::PI) / 360.0))
}

/// The pixels a cell spans at its zoom's split distance, at the reference
/// view: about six.
pub fn target_cell_px() -> f64 {
    px_per_unit(REFERENCE_VIEWPORT_H, REFERENCE_FOV_DEG) / SPLIT
}

/// Where ground starts being drawn with cells at `cell_zoom` at `Tau::FINEST`, going out.
pub const fn split_distance(cell_zoom: Zoom) -> f64 {
    SPLIT * units(cell_zoom)
}

/// The cell zoom a pixel covers at `r` from the eye: the widest cell whose
/// split distance is within `r`. Distance only: with the surface angle in,
/// the skyline would be drawn coarsest. Finer than the ground's near the
/// eye, held to what a zoom can name.
pub fn cell_at(r: f64) -> Zoom {
    (UNIT_Z as f64 - libm::floor(libm::log2(r / SPLIT))).clamp(0.0, Zoom::MAX as f64) as Zoom
}

/// The criterion's one scale factor, at least 1: every zoom's split
/// distance is `split_distance` / τ. The reach is sized at 1, and a
/// coarser τ only brings bands nearer, so none breaks it.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Tau(f64);

impl Tau {
    pub const FINEST: Tau = Tau(1.0);

    /// At least 1: a finer quality than the criterion breaks the reach.
    pub fn of(scale: f64) -> Tau {
        xpute_core::ensure!(scale >= 1.0, EINVAL);
        Tau(scale)
    }

    pub const fn scale(self) -> f64 {
        self.0
    }

    pub const fn split_distance(self, cell_zoom: Zoom) -> f64 {
        split_distance(cell_zoom) / self.0
    }

    pub fn cell_at(self, r: f64) -> Zoom {
        cell_at(r * self.0)
    }

    /// The tile zoom the ground is drawn from at `r`: the one whose observed
    /// cell is the rule's, held to the pyramid.
    pub fn z_at(self, r: f64) -> Zoom {
        (GROUND.z_of(self.cell_at(r)) - DRAWN_STEPS as i32).clamp(TOP_Z as i32, LEAF_Z as i32) as Zoom
    }

    pub const fn demand_reach(self, zoom: Zoom) -> f64 {
        demand_reach(zoom) / self.0
    }
}

/// The cell the rule measures a tile at `zoom` by, for a layer drawing its
/// tile's cells `drawn` zooms coarser than the rule's: the ground a step
/// (`DRAWN_STEPS`), a picture drawn a texel a cell none.
pub const fn observed_cell(zoom: Zoom, drawn: Zoom) -> Zoom {
    GROUND.cell_zoom(zoom) + drawn
}

/// How far from an observer's foot a demand at `z` can lie, for a layer
/// drawing `drawn` zooms coarser: the split distance of the observed cell a
/// zoom coarser. Bounds the run along the ground too.
pub const fn reach_of(zoom: Zoom, drawn: Zoom) -> f64 {
    split_distance(observed_cell(zoom - 1, drawn))
}

/// The ground's.
pub const fn demand_reach(zoom: Zoom) -> f64 {
    reach_of(zoom, DRAWN_STEPS)
}

/// The most tiles a zoom's demand span along an axis: ⌊d / tile⌋ + 2, the same at every zoom.
pub const fn reach_across(drawn: Zoom) -> u32 {
    (2.0 * reach_of(LEAF_Z, drawn) / units(LEAF_Z)) as u32 + 2
}

/// The most tiles a zoom's demand name about one observer.
pub const fn reach_tiles(drawn: Zoom) -> u32 {
    reach_across(drawn) * reach_across(drawn)
}

/// The ground's.
pub const REACH_ACROSS: u32 = reach_across(DRAWN_STEPS);
pub const REACH_TILES: u32 = reach_tiles(DRAWN_STEPS);

/// Every tile at `z` with ground within `radius` of `at`.
pub fn tiles_within(zoom: Zoom, at: [f64; 2], radius: f64) -> impl Iterator<Item = Tile> {
    let ys = tile_rows(zoom);
    squares_within(zoom, at, radius).filter(move |&[_, y]| ys.contains(&y)).map(move |xy| tile_at(zoom, xy))
}

/// `tiles_within` before X wraps.
pub fn squares_within(zoom: Zoom, at: [f64; 2], radius: f64) -> impl Iterator<Item = [i64; 2]> {
    xpute_core::ensure!(zoom >= TOP_Z, EINVAL);
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
#[path = "lod.test.rs"]
mod test;
