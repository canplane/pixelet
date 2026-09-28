// pxl-spec/space/lod.rs

//! Which level ground is drawn at from how far it is, and which pages of a
//! level an observation can ever fault on
//! (issue/20260928-what-is-drawn-is-what-the-screen-covers.md).
//!
//! Both are the lattice's, not a view's: the level is a function of a
//! distance alone, and the pages a fault can name are a function of a point
//! alone, so a bound on what is resident can be sized and proved before
//! anything is known of the world or the screen. How much past the faults is
//! fetched ahead, and when, is a prefetch policy over it; that no fault names
//! a page outside the bound is structure.

use super::pyramid::{page_at, span_of, LevelPage, GROUND, TOP_LEVEL};

/// The one criterion, as a distance per unit of a cell's side: a cell covers
/// the pixels a cell is allowed at `SPLIT` times its side from the eye, so a
/// level is drawn from `SPLIT` of its cells away out to twice that. 192 is
/// the ladder that was here before the criterion was stated in pixels — the
/// finest tier handing off at 192u at 1080 px and 50° (pxl-render
/// look/lod.rs, which reads it from here) — and a device that cannot afford
/// it chooses a larger one, which moves every level and every fault bound
/// together.
pub const SPLIT: f64 = 192.0;

/// Where ground stops being drawn at `level`: nearer than this, one of its
/// cells covers more than a cell's pixels. Any integer level, the ones finer
/// than z16 included.
pub const fn split_distance(level: i32) -> f64 {
    SPLIT * GROUND.scale(0).up(level).side()
}

/// The level whose cell a pixel covers at `r` units from the eye: the one
/// whose split distance `r` is past and the next one's it is not. A distance
/// and nothing else. A height's error shows on screen as its size over the
/// distance whatever the angle it is seen at, so the angle to the surface is
/// no part of it; with the angle in, the skyline, where that angle is most
/// oblique, would be drawn coarsest. Finer than z16 too — negative, z17 and
/// on — where the eye is nearer than z16's split distance; a layer whose
/// data ends coarser draws its finest there.
pub fn level(r: f64) -> i32 {
    (r / split_distance(0)).log2().floor() as i32
}

/// How far from an observer's foot a fault at `level` can lie, in units. An
/// observation that hits at a distance `r` asks for the page of `level(r)`
/// that holds the hit, and `level(r)` is `level` only nearer than the next
/// level's split distance — twice `level`'s own. Ground nearer than z16's is
/// asked at z16, terrain's finest, which is nearer still; ground past the
/// top's is not drawn (the far edge). The run along the ground is never
/// longer than the distance, so this bounds it horizontally. Nothing but the
/// distance enters: what the observation is — a pixel's ray, a point's probe
/// — and how what it answers is drawn are no part of it.
pub const fn fault_radius(level: u32) -> f64 {
    split_distance(level as i32 + 1)
}

/// The most pages a level's faults span along an axis: an interval of `d`
/// touches at most ⌊d / page⌋ + 2 of them, and the radius and a page grow by
/// the same factor a level, so it is the same at every level.
pub const FAULT_ACROSS: u32 = (2.0 * fault_radius(0) / span_of(0)) as u32 + 2;

/// The most pages a level's faults name about one observer: a disc touches
/// no more than the square about it does. What demand is sized for, a
/// level.
pub const FAULT_PAGES: u32 = FAULT_ACROSS * FAULT_ACROSS;

/// Every page of `level` with ground no farther than `radius` from `at`
/// along the ground, in units. With `fault_radius(level)` about an
/// observer's foot, every page a fault from it can name at that level, and
/// no more than `FAULT_PAGES`.
pub fn pages_within(level: u32, at: [f64; 2], radius: f64) -> impl Iterator<Item = LevelPage> {
    squares_within(level, at, radius).map(move |xy| page_at(level, xy))
}

/// The squares of `level`'s pages, by their column and row counted without
/// the seam, with ground no farther than `radius` from `at` along the ground:
/// `pages_within` before X wraps, for whoever measures a square from where
/// it lies rather than from the page it names.
pub fn squares_within(level: u32, at: [f64; 2], radius: f64) -> impl Iterator<Item = [i64; 2]> {
    xpute_core::ensure!(level <= TOP_LEVEL, EINVAL);
    let span = span_of(level);
    let rows = |c: f64| ((c - radius) / span).floor() as i64..=((c + radius) / span).floor() as i64;
    let (xs, ys) = (rows(at[0]), rows(at[1]));
    // How far a coordinate lies outside the page at `i` along an axis.
    let gap = move |c: f64, i: i64| (i as f64 * span - c).max(c - (i + 1) as f64 * span).max(0.0);
    ys.flat_map(move |y| xs.clone().map(move |x| [x, y]))
        .filter(move |&[x, y]| gap(at[0], x).hypot(gap(at[1], y)) <= radius)
}

#[cfg(test)]
#[path = "lod.test.rs"]
mod test;
