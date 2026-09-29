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

use super::lod::{cubes_meeting, fault_radius, Tau};
use super::pyramid::{page_at, span_of, LevelPage, TOP_LEVEL};
use super::v3::{Aabb, V3};

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
    /// top's end (`fault_radius(top)`), since no level answers farther.
    pub far: f64,
    /// The coarsest level the source supplies whole, as it declares itself
    /// (spec/pxl/spec/bake/published.json `SOURCE`): what every sample is answered
    /// at when nothing finer is, and no sample asks for coarser.
    pub top: u32,
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
    /// Where box `b` stands. Drawn, it stands up to `d² / 2R` lower at its
    /// farthest `d` along the planet's surface, so it is measured that much
    /// deeper: the planet has a down, and that is the one axis it names. A
    /// box without end is seen wherever it lies.
    pub fn measure(&self, b: &Aabb) -> Measure {
        let e = self.eye;
        let gap = |b: &Aabb, k: usize| (b.min[k] - e[k]).max(e[k] - b.max[k]).max(0.0);
        let reach = |b: &Aabb, k: usize| (e[k] - b.min[k]).abs().max((b.max[k] - e[k]).abs());
        let (along, reach_along) = (gap(b, 0).hypot(gap(b, 1)), reach(b, 0).hypot(reach(b, 1)));
        let mut drawn = *b;
        drawn.min[2] -= reach_along * reach_along / (2.0 * self.planet);
        let seen = !drawn.bounded() || self.frustum.is_none_or(|f| f.sees(drawn.center(), norm(drawn.extent()) / 2.0));
        Measure {
            seen,
            near: along.hypot(gap(&drawn, 2)),
            far: reach_along.hypot(reach(&drawn, 2)),
        }
    }

    /// The box every point drawn within `radius` of the eye is stored in:
    /// the eye's reach each way, and over it as far again as the planet
    /// draws ground that far away lower (`measure`).
    pub fn reach(&self, radius: f64) -> Aabb {
        let e = self.eye;
        Aabb {
            min: e.map(|c| c - radius),
            max: [e[0] + radius, e[1] + radius, e[2] + radius + radius * radius / (2.0 * self.planet)],
        }
    }
}

fn norm(v: V3) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// What an observation requires: what stands in `region` known at `level`
/// — the pyramid's level whose cell a sample there covers, z16 for any
/// finer, since no layer's pages are finer and a layer draws its finest
/// where more is asked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Requirement {
    pub level: u32,
    pub region: Aabb,
}

/// What a box of the world holds at a level, as far as what is known says.
/// How a layer stores it — a heightfield, bands of a column — is the
/// layer's, and nothing that asks sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Contents {
    /// Nothing is known of it: anything may stand anywhere in it.
    Unknown,
    /// Nothing stands in it.
    Empty,
    /// Everything that stands in it is inside `bound`, and what is drawn of
    /// it at the level can be off from what it stands for by up to `error`
    /// along each axis, never more than the box's own extent. The bound is
    /// conservative, not tight: tighter only saves work.
    Holds { bound: Aabb, error: V3 },
}

impl Contents {
    /// Where in `region`, the box it answers for, anything can stand: all of
    /// it where nothing is known.
    pub fn within(self, region: Aabb) -> Option<Aabb> {
        match self {
            Contents::Unknown => Some(region),
            Contents::Empty => None,
            Contents::Holds { bound, .. } => Some(bound),
        }
    }
}

/// What is known of the world: what a box holds at `level`, the resolution
/// it is asked at — the ground drawn at every level, the interpolation's
/// overshoot included, and whatever stands on it. Never less than the
/// truth, since an estimator's superset and every node a view makes rest on
/// it, and at a coarser level never less than at a finer one under it.
pub trait Bounds {
    fn contents(&self, level: u32, region: Aabb) -> Contents;
}

/// Nothing known anywhere: the bound every other is inside.
pub struct Unknown;

impl Bounds for Unknown {
    fn contents(&self, _level: u32, _region: Aabb) -> Contents {
        Contents::Unknown
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
    /// Every page of the requirement's level whose square is under the
    /// requirement's region, its edges its own on the south and west only,
    /// so a region over a page's square exactly is that page alone.
    fn pages(&self, r: &Requirement, each: &mut dyn FnMut(LevelPage)) {
        let span = span_of(r.level);
        let rows = |k: usize| {
            let first = (r.region.min[k] / span).floor() as i64;
            first..=((r.region.max[k] / span).ceil() as i64 - 1).max(first)
        };
        for y in rows(1) {
            for x in rows(0) {
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

/// The estimator from bounds alone, at a quality `tau`. A sample answered
/// `r` away requires its ground at `tau.level(r)`, no coarser than the
/// observation's `top`, which is `L` only for `r` in
/// `[tau.split_distance(L), tau.split_distance(L + 1))` — from nothing for
/// z16, which answers everything nearer too, and out to the far edge for the
/// top, which answers everything farther, since no level over it is supplied
/// — and nothing past the far edge. So a cube of `L`'s page lattice is
/// required only if some of what it holds can be sampled and its nearest
/// and farthest point from the eye leave some of that band between them.
/// That holds of every cube the exact demand has, whatever is hidden behind
/// what; one behind a hill is required too. What is required is nearer than
/// the band's end, and so along the ground too, which no `Tau` puts past
/// `fault_radius(L)` — so no level requires more than the fault bound's
/// `FAULT_PAGES` pages.
pub struct ByBounds {
    pub tau: Tau,
}

/// The distances `level` is required over at `tau` for `o`: from its split
/// distance — from nothing for z16, which answers everything nearer too —
/// to the next level's, or to the far edge for the observation's top, which
/// answers everything farther. None where the level begins past the far
/// edge. Every estimator's bands, so none requires a level farther than
/// another would.
pub fn band(o: &Observation, tau: Tau, level: u32) -> Option<(f64, f64)> {
    let from = if level == 0 { 0.0 } else { tau.split_distance(level as i32) };
    let to = if level == o.top { fault_radius(level) } else { tau.fault_radius(level) };
    (from <= o.far).then_some((from, to))
}

impl DemandEstimator for ByBounds {
    fn requirements(&self, o: &Observation, bounds: &impl Bounds, each: &mut dyn FnMut(Requirement)) {
        xpute_core::ensure!(o.top <= TOP_LEVEL && o.far <= fault_radius(o.top), EINVAL);
        for level in 0..=o.top {
            let Some((from, to)) = band(o, self.tau, level) else { break };
            let span = span_of(level);
            for at in cubes_meeting(span, o.reach(to.min(o.far))) {
                let region = Aabb {
                    min: at.map(|c| c as f64 * span),
                    max: at.map(|c| (c + 1) as f64 * span),
                };
                let Some(held) = bounds.contents(level, region).within(region) else { continue };
                let m = o.measure(&held);
                if m.seen && m.near < to && m.near <= o.far && m.far >= from {
                    each(Requirement { level, region });
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "observation.test.rs"]
mod test;
