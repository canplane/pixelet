// pxl-spec/space/lod.rs

//! The level ground is drawn at from its distance, and the pages a level's
//! faults can name about a point. Both depend only on the lattice, so the
//! resident bound is proved before anything of the world or screen is known.

use super::pvf3::REACH;
use super::pyramid::{page_at, span_of, LevelPage, GROUND, TOP_LEVEL};
use super::v3::Aabb;

/// The criterion as a distance per cell side: a level is drawn from `SPLIT`
/// of its cells away out to twice that. 192 hands the finest tier off at
/// 192u at 1080 px and 50°.
pub const SPLIT: f64 = 192.0;

/// Where ground stops being drawn at `level` at `Tau::FINEST`; any integer level.
pub const fn split_distance(level: i32) -> f64 {
    SPLIT * GROUND.scale(0).up(level).side()
}

/// The level whose cell a pixel covers at `r` from the eye. Distance only:
/// with the surface angle in, the skyline would be drawn coarsest. Negative
/// (finer than z16) near the eye.
pub fn level(r: f64) -> i32 {
    (r / split_distance(0)).log2().floor() as i32
}

/// The criterion's one scale factor, at least 1: every level's split
/// distance is `split_distance` / τ. The fault bound is sized at 1, and a
/// coarser τ only brings bands nearer, so none breaks it.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Tau(f64);

impl Tau {
    pub const FINEST: Tau = Tau(1.0);

    /// At least 1.
    pub const fn of(scale: f64) -> Tau {
        assert!(scale >= 1.0, "a quality finer than the criterion the fault bound is sized at");
        Tau(scale)
    }

    pub const fn scale(self) -> f64 {
        self.0
    }

    pub const fn split_distance(self, level: i32) -> f64 {
        split_distance(level) / self.0
    }

    pub fn level(self, r: f64) -> i32 {
        level(r * self.0)
    }

    pub const fn fault_radius(self, level: u32) -> f64 {
        self.split_distance(level as i32 + 1)
    }
}

/// How far from an observer's foot a fault at `level` can lie: the next
/// level's split distance. Bounds the run along the ground too.
pub const fn fault_radius(level: u32) -> f64 {
    split_distance(level as i32 + 1)
}

/// The most pages a level's faults span along an axis: ⌊d / page⌋ + 2, the same at every level.
pub const FAULT_ACROSS: u32 = (2.0 * fault_radius(0) / span_of(0)) as u32 + 2;

/// The most pages a level's faults name about one observer.
pub const FAULT_PAGES: u32 = FAULT_ACROSS * FAULT_ACROSS;

/// Every page of `level` with ground within `radius` of `at`.
pub fn pages_within(level: u32, at: [f64; 2], radius: f64) -> impl Iterator<Item = LevelPage> {
    squares_within(level, at, radius).map(move |xy| page_at(level, xy))
}

/// `pages_within` before X wraps.
pub fn squares_within(level: u32, at: [f64; 2], radius: f64) -> impl Iterator<Item = [i64; 2]> {
    xpute_core::ensure!(level <= TOP_LEVEL, EINVAL);
    let span = span_of(level);
    let rows = |c: f64| ((c - radius) / span).floor() as i64..=((c + radius) / span).floor() as i64;
    let (xs, ys) = (rows(at[0]), rows(at[1]));
    let gap = move |c: f64, i: i64| (i as f64 * span - c).max(c - (i + 1) as f64 * span).max(0.0);
    ys.flat_map(move |y| xs.clone().map(move |x| [x, y]))
        .filter(move |&[x, y]| gap(at[0], x).hypot(gap(at[1], y)) <= radius)
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
