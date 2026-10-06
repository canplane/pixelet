// pxl-spec/space/coord/v3.rs

//! Three doubles, and the arithmetic over them. A V3 is a value, not storage,
//! so each returns its result and the assignment says where it lands; what
//! fills a buffer takes the buffer instead.

pub type V3 = [f64; 3];

pub fn scale3(v: &V3, k: f64) -> V3 {
    [v[0] * k, v[1] * k, v[2] * k]
}

pub fn add3(v: &V3, w: &V3) -> V3 {
    [v[0] + w[0], v[1] + w[1], v[2] + w[2]]
}
pub fn sub3(v: &V3, w: &V3) -> V3 {
    [v[0] - w[0], v[1] - w[1], v[2] - w[2]]
}

pub fn dot3(v: &V3, w: &V3) -> f64 {
    v[0] * w[0] + v[1] * w[1] + v[2] * w[2]
}
pub fn len3(v: &V3) -> f64 {
    libm::hypot(libm::hypot(v[0], v[1]), v[2])
}
pub fn cross3(v: &V3, w: &V3) -> V3 {
    [v[1] * w[2] - v[2] * w[1], v[2] * w[0] - v[0] * w[2], v[0] * w[1] - v[1] * w[0]]
}
/// A zero vector stays zero.
pub fn norm3(v: &V3) -> V3 {
    let len = match len3(v) {
        0.0 => 1.0,
        l => l,
    };
    [v[0] / len, v[1] / len, v[2] / len]
}

pub fn floor3n(v: &V3, k: f64) -> V3 {
    [(v[0] / k).floor() * k, (v[1] / k).floor() * k, (v[2] / k).floor() * k]
}
pub fn ceil3n(v: &V3, k: f64) -> V3 {
    [(v[0] / k).ceil() * k, (v[1] / k).ceil() * k, (v[2] / k).ceil() * k]
}
/// The middle of the cell of side `k` holding `v`.
pub fn center3n(v: &V3, k: f64) -> V3 {
    floor3n(v, k).map(|c| c + k * 0.5)
}

/// An axis-aligned box, in units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: V3,
    pub max: V3,
}

impl Aabb {
    /// The part inside `other`, closed; none only where they miss.
    pub fn meet(self, other: Aabb) -> Option<Aabb> {
        let min: V3 = std::array::from_fn(|k| self.min[k].max(other.min[k]));
        let max: V3 = std::array::from_fn(|k| self.max[k].min(other.max[k]));
        (0..3).all(|k| min[k] <= max[k]).then_some(Aabb { min, max })
    }

    pub fn join(self, other: Aabb) -> Aabb {
        Aabb {
            min: std::array::from_fn(|k| self.min[k].min(other.min[k])),
            max: std::array::from_fn(|k| self.max[k].max(other.max[k])),
        }
    }

    pub fn extent(self) -> V3 {
        std::array::from_fn(|k| self.max[k] - self.min[k])
    }

    pub fn center(self) -> V3 {
        std::array::from_fn(|k| (self.min[k] + self.max[k]) / 2.0)
    }

    /// The bounding sphere's, about `center`.
    pub fn radius(self) -> f64 {
        len3(&self.extent()) / 2.0
    }

    /// The slab test: where the ray enters and leaves, unclipped, and the axis
    /// it enters across. An axis the ray does not move along is inside where
    /// the origin is.
    pub fn span_along(self, origin: V3, direction: V3) -> Option<(f64, f64, usize)> {
        let (mut enter, mut leave, mut axis) = (f64::NEG_INFINITY, f64::INFINITY, 0);
        for k in 0..3 {
            if direction[k].abs() < 1e-12 {
                if origin[k] < self.min[k] || origin[k] > self.max[k] {
                    return None;
                }
                continue;
            }
            let (a, b) = ((self.min[k] - origin[k]) / direction[k], (self.max[k] - origin[k]) / direction[k]);
            let (near, far) = if direction[k] > 0.0 { (a, b) } else { (b, a) };
            if near > enter {
                (enter, axis) = (near, k);
            }
            leave = leave.min(far);
        }
        (enter <= leave).then_some((enter, leave, axis))
    }

    /// Closed.
    pub fn holds(self, p: V3) -> bool {
        (0..3).all(|k| self.min[k] <= p[k] && p[k] <= self.max[k])
    }

    pub fn contains(self, inner: Aabb) -> bool {
        self.holds(inner.min) && self.holds(inner.max)
    }

    pub fn moved(self, by: V3) -> Aabb {
        Aabb {
            min: std::array::from_fn(|k| self.min[k] + by[k]),
            max: std::array::from_fn(|k| self.max[k] + by[k]),
        }
    }

    /// Grown outward to whole cells of `side`.
    pub fn snapped_out(self, side: f64) -> Aabb {
        Aabb {
            min: floor3n(&self.min, side),
            max: ceil3n(&self.max, side),
        }
    }

    pub fn nearest(self, p: V3) -> V3 {
        std::array::from_fn(|k| p[k].clamp(self.min[k], self.max[k]))
    }

    pub fn bounded(self) -> bool {
        self.min.iter().chain(&self.max).all(|c| c.is_finite())
    }
}
