// pxl-spec/space/observation.rs

//! What an observation is and what it requires, before anything is a page
//! (issue/20260928-what-is-drawn-is-what-the-screen-covers.md).
//!
//! An observation's meaning is the first visible answer for each angular
//! sample: an eye, a direction inside what it looks at, a footprint no wider
//! than the criterion allows (lod.rs `SPLIT`, one for every observation).
//! Ray casting and rasterizing with a depth buffer are two solvers of it; it
//! is neither. What a sample requires is information at a resolution — the
//! level `lod::level` gives the distance it is answered at — over the ground
//! the answer lies on: a `Requirement`, which names no page. A layer's
//! storage maps a requirement to the pages that meet it (`Storage`), so a
//! page's shape can change and the observation's law does not.
//!
//! The exact demand is a function of what the world holds, which is not
//! known before it is held. An estimator names a superset of it from what
//! is known (`DemandEstimator`): never less, since a requirement left out is
//! ground that never refines, and more only costs fetches. The first works
//! from bounds alone (`ByBounds`) and asks for hidden ground too; one that
//! casts the samples against the bounds, or reads the GPU's feedback, is a
//! smaller superset, and none of them changes what an observation means.
//! How the answers are drawn — chunks, meshes, their sizes — is no input to
//! any of it.

use super::lod::{fault_radius, split_distance, squares_within};
use super::pyramid::{page_at, span_of, LevelPage, TOP_LEVEL};

/// A square of ground: its south-west corner and its side, in units, as a
/// page's `rect` is.
pub type Rect = (f64, f64, f64);

/// What an observation looks at: six planes, normals in, `n · x + d ≥ 0`
/// inside, in world units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frustum {
    pub planes: [f64; 24],
}

impl Frustum {
    /// Whether any of a sphere at `center` of `radius` can be inside every
    /// plane. Conservative: a sphere near a corner can pass every plane and
    /// still lie outside.
    pub fn sees(&self, center: [f64; 3], radius: f64) -> bool {
        self.planes.as_chunks::<4>().0.iter().all(|p| p[0] * center[0] + p[1] * center[1] + p[2] * center[2] + p[3] >= -radius)
    }
}

/// An observer and what it samples.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Observation {
    /// Where the samples are taken from, in units.
    pub eye: [f64; 3],
    /// The directions sampled, or every direction where none: a probe all
    /// around, as a point observer's is.
    pub frustum: Option<Frustum>,
    /// How far a sample is answered: past it nothing is drawn. Never past the
    /// top level's end (`fault_radius(TOP_LEVEL)`), since no level answers
    /// farther.
    pub far: f64,
    /// The planet's radius, in units: ground `d` from the eye is drawn
    /// `d² / 2R` under where it is stored.
    pub planet: f64,
    /// Where the ground under the eye stands, as the observer last knew it:
    /// what its height is measured from. No part of what it requires.
    pub ground: f64,
}

/// Where a box stands against an observation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    /// Whether any of it can be sampled.
    pub seen: bool,
    /// Its nearest and farthest point from the eye.
    pub near: f64,
    pub far: f64,
}

impl Observation {
    /// Where a box of ground stands: `ground` and the heights `z` it spans,
    /// or where they are not known, as tall as anything could stand there —
    /// seen wherever it lies, as near as its footprint and without end.
    /// Drawn, it stands up to `d² / 2R` lower at its farthest `d` along the
    /// ground, so it is measured that much deeper.
    pub fn measure(&self, (x0, y0, side): Rect, z: Option<(f64, f64)>) -> Measure {
        let [ex, ey, ez] = self.eye;
        let gap = |e: f64, lo: f64| (lo - e).max(e - lo - side).max(0.0);
        let reach = |e: f64, lo: f64| (e - lo).abs().max((lo + side - e).abs());
        let (along, reach_along) = (gap(ex, x0).hypot(gap(ey, y0)), reach(ex, x0).hypot(reach(ey, y0)));
        let Some((zlo, zhi)) = z else {
            return Measure {
                seen: true,
                near: along,
                far: f64::INFINITY,
            };
        };
        let zlo = zlo - reach_along * reach_along / (2.0 * self.planet);
        let zgap = (zlo - ez).max(ez - zhi).max(0.0);
        let zreach = (ez - zlo).abs().max((zhi - ez).abs());
        let seen = self.frustum.is_none_or(|f| {
            let center = [x0 + side / 2.0, y0 + side / 2.0, (zlo + zhi) / 2.0];
            f.sees(center, (2.0 * side * side + (zhi - zlo).powi(2)).sqrt() / 2.0)
        });
        Measure {
            seen,
            near: along.hypot(zgap),
            far: reach_along.hypot(zreach),
        }
    }
}

/// What an observation requires: the ground over `ground` known at `level`
/// — the pyramid's level whose cell a sample there covers, z16 for any
/// finer, since no layer's pages are finer and a layer draws its finest
/// where more is asked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Requirement {
    pub level: u32,
    pub ground: Rect,
}

/// What is known of the heights over a piece of ground: the lowest and
/// highest anything there can stand — the ground drawn at every level, the
/// interpolation's overshoot included, and whatever stands on it — or none
/// where nothing is known. Never tighter than the truth: an estimator's
/// superset rests on it. `level` is the resolution the ground is asked at,
/// for a reader that keeps its bounds by level.
pub trait Bounds {
    fn heights(&self, level: u32, ground: Rect) -> Option<(f64, f64)>;
}

/// Nothing known anywhere: the bound every other is inside.
pub struct Unknown;

impl Bounds for Unknown {
    fn heights(&self, _level: u32, _ground: Rect) -> Option<(f64, f64)> {
        None
    }
}

/// How a layer's storage meets a requirement.
pub trait Storage {
    /// Each page that holds the requirement's ground at its level.
    fn pages(&self, r: &Requirement, each: &mut dyn FnMut(LevelPage));
}

/// The pyramid's pages, as terrain and places keep them: a level's pages
/// side by side, 2D, whatever the heights.
pub struct Paged;

impl Storage for Paged {
    /// Every page of the requirement's level with ground under the
    /// requirement's, its edges its own on the south and west only, so
    /// ground a page's square exactly is that page alone.
    fn pages(&self, r: &Requirement, each: &mut dyn FnMut(LevelPage)) {
        let span = span_of(r.level);
        let (x0, y0, side) = r.ground;
        let rows = |lo: f64| {
            let first = (lo / span).floor() as i64;
            first..=(((lo + side) / span).ceil() as i64 - 1).max(first)
        };
        for y in rows(y0) {
            for x in rows(x0) {
                each(page_at(r.level, [x, y]));
            }
        }
    }
}

/// Names what an observation may require: a superset of what its samples,
/// answered by the world as it is, would.
pub trait DemandEstimator {
    /// Each requirement `o` may have, as far as `bounds` know the world.
    fn requirements(&self, o: &Observation, bounds: &impl Bounds, each: &mut dyn FnMut(Requirement));
}

/// The estimator from bounds alone. A sample answered `r` away requires its
/// ground at `level(r)`, which is `L` only for `r` in `[split_distance(L),
/// 2 · split_distance(L))` — from nothing for z16, which answers everything
/// nearer too — and nothing past the far edge. So a square of `L`'s is
/// required only if some of it can be sampled and its nearest and farthest
/// point from the eye leave some of that band between them. That holds of
/// every square the exact demand has, whatever is hidden behind what; one
/// behind a hill is required too. The squares are the lattice's at `L`,
/// found among those within the band's end along the ground, so no level
/// requires more than the fault bound's `FAULT_PAGES` of them.
pub struct ByBounds;

impl DemandEstimator for ByBounds {
    fn requirements(&self, o: &Observation, bounds: &impl Bounds, each: &mut dyn FnMut(Requirement)) {
        xpute_core::ensure!(o.far <= fault_radius(TOP_LEVEL), EINVAL);
        for level in 0..=TOP_LEVEL {
            let from = if level == 0 { 0.0 } else { split_distance(level as i32) };
            let to = fault_radius(level);
            if from > o.far {
                break;
            }
            let span = span_of(level);
            for [x, y] in squares_within(level, [o.eye[0], o.eye[1]], to.min(o.far)) {
                let ground = (x as f64 * span, y as f64 * span, span);
                let m = o.measure(ground, bounds.heights(level, ground));
                if m.seen && m.near < to && m.near <= o.far && m.far >= from {
                    each(Requirement { level, ground });
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "observation.test.rs"]
mod test;
