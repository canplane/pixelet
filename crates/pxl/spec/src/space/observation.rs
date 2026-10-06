// pxl-spec/space/observation.rs

//! What an observation requires, before anything is a tile: information at a
//! zoom over a region (`Requirement`). A layer's storage maps a requirement
//! to tiles (`Storage`). An estimator names a superset of the exact demand
//! from what is known (`DemandEstimator`); never less, since a requirement
//! left out is ground that never refines.

use super::lod::{cubes_meeting, demand_reach, Tau};
use super::pyramid::{tile_at, tile_rows, Tile, GROUND, LEAF_Z, TOP_Z};
use super::v3::{dot3, Aabb, V3};
use super::zoom::nearest_box;
use super::zoom::{units, Zoom};
use crate::bake::constant::CELL;

/// Six planes, normals in, `n · x + d ≥ 0` inside, in world units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frustum {
    pub planes: [f64; 24],
}

impl Frustum {
    /// Conservative: a sphere near a corner can pass every plane and still lie outside.
    pub fn sees(&self, center: V3, radius: f64) -> bool {
        self.planes.as_chunks::<4>().0.iter().all(|p| dot3(&[p[0], p[1], p[2]], &center) + p[3] >= -radius)
    }

    /// The same planes for points measured from `at`.
    pub fn about(mut self, at: V3) -> Frustum {
        for p in self.planes.as_chunks_mut::<4>().0 {
            p[3] += dot3(&[p[0], p[1], p[2]], &at);
        }
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Observation {
    pub eye: [f64; 3],
    /// None for every direction.
    pub frustum: Option<Frustum>,
    /// Never past `demand_reach(top)`.
    pub far: f64,
    /// The coarsest zoom the source supplies whole; no sample asks for coarser.
    pub top: Zoom,
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
        let b = &nearest_box(b, e[0]);
        let gap = |b: &Aabb, k: usize| (b.min[k] - e[k]).max(e[k] - b.max[k]).max(0.0);
        let reach = |b: &Aabb, k: usize| (e[k] - b.min[k]).abs().max((b.max[k] - e[k]).abs());
        let (along, reach_along) = (libm::hypot(gap(b, 0), gap(b, 1)), libm::hypot(reach(b, 0), reach(b, 1)));
        let mut drawn = *b;
        drawn.min[2] -= reach_along * reach_along / (2.0 * self.planet);
        let seen = !drawn.bounded() || self.frustum.is_none_or(|f| f.sees(drawn.center(), drawn.radius()));
        Measure {
            seen,
            near: libm::hypot(along, gap(&drawn, 2)),
            far: libm::hypot(reach_along, reach(&drawn, 2)),
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

/// What is known of the world. Never less than the truth, and at a coarser
/// zoom never less than at a finer one under it.
pub trait Bounds {
    fn contents(&self, zoom: Zoom, region: Aabb) -> Contents;

    /// The finest cell zoom anything in `region` is held at: the ground's
    /// leaf cell unless a layer finer stands there.
    fn finest(&self, _region: Aabb) -> Zoom {
        CELL
    }
}

pub struct Unknown;

impl Bounds for Unknown {
    fn contents(&self, _zoom: Zoom, _region: Aabb) -> Contents {
        Contents::Unknown
    }
}

pub trait Storage {
    fn tiles(&self, r: &Requirement, each: &mut dyn FnMut(Tile));
}

/// The pyramid's 2D tiles, as terrain and places keep them.
pub struct Tiled;

impl Storage for Tiled {
    /// Every tile under the region; edges are inclusive south and west only.
    fn tiles(&self, r: &Requirement, each: &mut dyn FnMut(Tile)) {
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
}

/// Names a superset of what an observation's samples require.
pub trait DemandEstimator {
    fn requirements(&self, o: &Observation, bounds: &impl Bounds, each: &mut dyn FnMut(Requirement));
}

/// The estimator from bounds alone: a cube at zoom `z` is required when some
/// of what it holds can be seen and its distance range overlaps `z`'s band.
/// Hidden cubes are required too.
pub struct ByBounds {
    pub tau: Tau,
}

/// The distances `z` is required over: its split distance (0 for the leaves) to the
/// next coarser zoom's, or to the far edge for the top. None past the far edge.
pub fn band(o: &Observation, tau: Tau, zoom: Zoom) -> Option<(f64, f64)> {
    let from = if zoom == LEAF_Z { 0.0 } else { tau.split_distance(GROUND.grid_cell(zoom)) };
    let to = if zoom == o.top { demand_reach(zoom) } else { tau.demand_reach(zoom) };
    (from <= o.far).then_some((from, to))
}

impl DemandEstimator for ByBounds {
    fn requirements(&self, o: &Observation, bounds: &impl Bounds, each: &mut dyn FnMut(Requirement)) {
        xpute_core::ensure!(o.top >= TOP_Z && o.far <= demand_reach(o.top), EINVAL);
        for zoom in (o.top..=LEAF_Z).rev() {
            let Some((from, to)) = band(o, self.tau, zoom) else { break };
            let span = units(zoom);
            for at in cubes_meeting(span, o.reach(to.min(o.far))) {
                let region = Aabb {
                    min: at.map(|c| c as f64 * span),
                    max: at.map(|c| (c + 1) as f64 * span),
                };
                let Some(held) = bounds.contents(zoom, region).within(region) else { continue };
                let m = o.measure(&held);
                if m.seen && m.near < to && m.near <= o.far && m.far >= from {
                    each(Requirement { zoom, region });
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "observation.test.rs"]
mod test;
