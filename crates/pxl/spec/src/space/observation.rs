// pxl-spec/space/observation.rs

//! What an observation requires, before anything is a page: information at a
//! level over a region (`Requirement`). A layer's storage maps a requirement
//! to pages (`Storage`). An estimator names a superset of the exact demand
//! from what is known (`DemandEstimator`); never less, since a requirement
//! left out is ground that never refines.

use super::lod::{cubes_meeting, fault_radius, Tau};
use super::pyramid::{page_at, span_of, LevelPage, TOP_LEVEL};
use super::v3::{Aabb, V3};

/// Six planes, normals in, `n · x + d ≥ 0` inside, in world units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frustum {
    pub planes: [f64; 24],
}

impl Frustum {
    /// Conservative: a sphere near a corner can pass every plane and still lie outside.
    pub fn sees(&self, center: [f64; 3], radius: f64) -> bool {
        self.planes.as_chunks::<4>().0.iter().all(|p| p[0] * center[0] + p[1] * center[1] + p[2] * center[2] + p[3] >= -radius)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Observation {
    pub eye: [f64; 3],
    /// None for every direction.
    pub frustum: Option<Frustum>,
    /// Never past `fault_radius(top)`.
    pub far: f64,
    /// The coarsest level the source supplies whole; no sample asks for coarser.
    pub top: u32,
    /// In units: ground `d` from the eye is drawn `d² / 2R` under where it is stored.
    pub planet: f64,
    /// The ground under the eye as last known; not part of what is required.
    pub ground: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    pub seen: bool,
    pub near: f64,
    pub far: f64,
}

impl Observation {
    /// Where box `b` stands, measured `d² / 2R` deeper to account for the
    /// planet's curvature. An unbounded box is always seen.
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

    /// The box every point drawn within `radius` of the eye is stored in, curvature included.
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

/// What stands in `region` known at `level`; z16 for anything finer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Requirement {
    pub level: u32,
    pub region: Aabb,
}

/// What a box holds at a level, as far as is known.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Contents {
    Unknown,
    Empty,
    /// Everything is inside `bound`, drawn off by up to `error` per axis.
    /// Conservative, not tight.
    Holds { bound: Aabb, error: V3 },
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

/// What is known of the world. Never less than the truth, and at a coarser
/// level never less than at a finer one under it.
pub trait Bounds {
    fn contents(&self, level: u32, region: Aabb) -> Contents;
}

pub struct Unknown;

impl Bounds for Unknown {
    fn contents(&self, _level: u32, _region: Aabb) -> Contents {
        Contents::Unknown
    }
}

pub trait Storage {
    fn pages(&self, r: &Requirement, each: &mut dyn FnMut(LevelPage));
}

/// The pyramid's 2D pages, as terrain and places keep them.
pub struct Paged;

impl Storage for Paged {
    /// Every page under the region; edges are inclusive south and west only.
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

/// Names a superset of what an observation's samples require.
pub trait DemandEstimator {
    fn requirements(&self, o: &Observation, bounds: &impl Bounds, each: &mut dyn FnMut(Requirement));
}

/// The estimator from bounds alone: a cube of level `L` is required when some
/// of what it holds can be seen and its distance range overlaps `L`'s band.
/// Hidden cubes are required too.
pub struct ByBounds {
    pub tau: Tau,
}

/// The distances `level` is required over: its split distance (0 for z16) to
/// the next level's, or to the far edge for the top. None past the far edge.
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
