// pxl-spec/space/v3.rs

//! Three doubles, and the arithmetic over them, each returning its result.

pub type V3 = [f64; 3];

pub fn fill3(e: f64) -> V3 {
    [e, e, e]
}

pub fn set3(x: f64, y: f64, z: f64) -> V3 {
    [x, y, z]
}

pub fn copy3(from: &V3) -> V3 {
    [from[0], from[1], from[2]]
}

pub fn neg3(v: &V3) -> V3 {
    [-v[0], -v[1], -v[2]]
}
pub fn abs3(v: &V3) -> V3 {
    [v[0].abs(), v[1].abs(), v[2].abs()]
}

pub fn add3(v: &V3, w: &V3) -> V3 {
    [v[0] + w[0], v[1] + w[1], v[2] + w[2]]
}
pub fn add3n(v: &V3, k: f64) -> V3 {
    [v[0] + k, v[1] + k, v[2] + k]
}
pub fn sub3(v: &V3, w: &V3) -> V3 {
    [v[0] - w[0], v[1] - w[1], v[2] - w[2]]
}
pub fn sub3n(v: &V3, k: f64) -> V3 {
    [v[0] - k, v[1] - k, v[2] - k]
}
pub fn mul3(v: &V3, w: &V3) -> V3 {
    [v[0] * w[0], v[1] * w[1], v[2] * w[2]]
}
pub fn mul3n(v: &V3, k: f64) -> V3 {
    [v[0] * k, v[1] * k, v[2] * k]
}
pub fn div3(v: &V3, w: &V3) -> V3 {
    [v[0] / w[0], v[1] / w[1], v[2] / w[2]]
}
pub fn div3n(v: &V3, k: f64) -> V3 {
    [v[0] / k, v[1] / k, v[2] / k]
}
pub fn mod3(v: &V3, w: &V3) -> V3 {
    [v[0] % w[0], v[1] % w[1], v[2] % w[2]]
}
pub fn mod3n(v: &V3, k: f64) -> V3 {
    [v[0] % k, v[1] % k, v[2] % k]
}

pub fn dot3(v: &V3, w: &V3) -> f64 {
    v[0] * w[0] + v[1] * w[1] + v[2] * w[2]
}
pub fn len3(v: &V3) -> f64 {
    dot3(v, v).sqrt()
}
pub fn cross3(v: &V3, w: &V3) -> V3 {
    [v[1] * w[2] - v[2] * w[1], v[2] * w[0] - v[0] * w[2], v[0] * w[1] - v[1] * w[0]]
}
pub fn norm3(v: &V3) -> V3 {
    let len = match len3(v) {
        0.0 => 1.0,
        l => l,
    };
    div3n(v, len)
}

use xpute_core::math::scalar::round;

pub fn floor3(base: &V3, span: &V3) -> V3 {
    [(base[0] / span[0]).floor() * span[0], (base[1] / span[1]).floor() * span[1], (base[2] / span[2]).floor() * span[2]]
}
pub fn floor3n(v: &V3, k: f64) -> V3 {
    [(v[0] / k).floor() * k, (v[1] / k).floor() * k, (v[2] / k).floor() * k]
}
pub fn center3(base: &V3, span: &V3) -> V3 {
    add3(&floor3(base, span), &mul3n(span, 0.5))
}
pub fn center3n(v: &V3, k: f64) -> V3 {
    add3n(&floor3n(v, k), k * 0.5)
}
pub fn ceil3(base: &V3, span: &V3) -> V3 {
    [(base[0] / span[0]).ceil() * span[0], (base[1] / span[1]).ceil() * span[1], (base[2] / span[2]).ceil() * span[2]]
}
pub fn ceil3n(v: &V3, k: f64) -> V3 {
    [(v[0] / k).ceil() * k, (v[1] / k).ceil() * k, (v[2] / k).ceil() * k]
}
pub fn round3(base: &V3, span: &V3) -> V3 {
    [round(base[0] / span[0]) * span[0], round(base[1] / span[1]) * span[1], round(base[2] / span[2]) * span[2]]
}
pub fn round3n(v: &V3, k: f64) -> V3 {
    [round(v[0] / k) * k, round(v[1] / k) * k, round(v[2] / k) * k]
}

/// `x | 0`: ToInt32 — the fraction dropped, modulo 2^32, NaN to 0.
fn to_int32(x: f64) -> i32 {
    if x.is_finite() {
        (x.trunc() % 4_294_967_296.0) as i64 as i32
    } else {
        0
    }
}

pub fn i3(v: &V3) -> V3 {
    [to_int32(v[0]) as f64, to_int32(v[1]) as f64, to_int32(v[2]) as f64]
}

pub fn snap3n(v: &V3, mask: i32) -> V3 {
    [(to_int32(v[0]) & !mask) as f64, (to_int32(v[1]) & !mask) as f64, (to_int32(v[2]) & !mask) as f64]
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
