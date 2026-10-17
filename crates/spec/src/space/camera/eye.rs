// pxl-spec/space/camera/eye.rs

//! The observer: where the eye is, what it looks at and how far it sees.

use crate::space::coord::aabb::Aabb;
use crate::space::coord::v3::{self, V3};
use crate::space::coord::world::nearest_box;

/// Six planes, normals in, `n · x + d ≥ 0` inside, in world units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frustum {
    pub planes: [f64; 24],
}

impl Frustum {
    /// Whether a sphere may show: for a round bound (a box is `sees_box`'s).
    /// Conservative: a sphere near a corner can pass every plane and still
    /// lie outside.
    pub fn sees(&self, center: V3, radius: f64) -> bool {
        self.planes.as_chunks::<4>().0.iter().all(|p| v3::dot(&[p[0], p[1], p[2]], &center) + p[3] >= -radius)
    }

    /// Whether the box from `min` to `max` may show: each plane has its corner
    /// farthest along the plane's normal inside. Conservative as `sees` is,
    /// but no wider than the box.
    pub fn sees_box(&self, min: V3, max: V3) -> bool {
        self.planes.as_chunks::<4>().0.iter().all(|p| {
            let far: V3 = core::array::from_fn(|k| if p[k] >= 0.0 { max[k] } else { min[k] });
            v3::dot(&[p[0], p[1], p[2]], &far) + p[3] >= 0.0
        })
    }

    /// The same planes for points measured from `at`.
    pub fn about(mut self, at: V3) -> Frustum {
        for p in self.planes.as_chunks_mut::<4>().0 {
            p[3] += v3::dot(&[p[0], p[1], p[2]], &at);
        }
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Observation {
    pub eye: [f64; 3],
    /// None for every direction.
    pub frustum: Option<Frustum>,
    /// How far the layers' demand reaches: never past the top's `Lattice::demand_reach`.
    pub reach: f64,
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
    /// Where box `b` stands from the eye, where it is drawn: what is drawn
    /// stands where it is stored. An unbounded box is always seen.
    pub fn measure(&self, b: &Aabb) -> Measure {
        let e = self.eye;
        let b = &nearest_box(b, e[0]);
        let gap = |k: usize| (b.min[k] - e[k]).max(e[k] - b.max[k]).max(0.0);
        let reach = |k: usize| (e[k] - b.min[k]).abs().max((b.max[k] - e[k]).abs());
        let seen = !b.bounded() || self.frustum.is_none_or(|f| f.sees_box(b.min, b.max));
        Measure {
            seen,
            near: libm::hypot(libm::hypot(gap(0), gap(1)), gap(2)),
            far: libm::hypot(libm::hypot(reach(0), reach(1)), reach(2)),
        }
    }

    /// The box every point drawn within `radius` of the eye is stored in.
    pub fn reach(&self, radius: f64) -> Aabb {
        let e = self.eye;
        Aabb {
            min: e.map(|c| c - radius),
            max: e.map(|c| c + radius),
        }
    }
}
