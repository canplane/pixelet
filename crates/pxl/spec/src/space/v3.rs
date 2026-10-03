// pxl-spec/space/v3.rs

//! Three doubles, and the arithmetic over them. Every function writes into
//! the `out` it is given and returns it; nothing here allocates.

pub type V3 = [f64; 3];

pub fn fill3(e: f64, out: &mut V3) -> &mut V3 {
    out[0] = e;
    out[1] = e;
    out[2] = e;
    out
}

pub fn set3(x: f64, y: f64, z: f64, out: &mut V3) -> &mut V3 {
    out[0] = x;
    out[1] = y;
    out[2] = z;
    out
}

pub fn copy3<'a>(from: &V3, out: &'a mut V3) -> &'a mut V3 {
    out[0] = from[0];
    out[1] = from[1];
    out[2] = from[2];
    out
}

pub fn neg3<'a>(v: &V3, out: &'a mut V3) -> &'a mut V3 {
    out[0] = -v[0];
    out[1] = -v[1];
    out[2] = -v[2];
    out
}
pub fn abs3<'a>(v: &V3, out: &'a mut V3) -> &'a mut V3 {
    out[0] = v[0].abs();
    out[1] = v[1].abs();
    out[2] = v[2].abs();
    out
}

pub fn add3<'a>(v: &V3, w: &V3, out: &'a mut V3) -> &'a mut V3 {
    out[0] = v[0] + w[0];
    out[1] = v[1] + w[1];
    out[2] = v[2] + w[2];
    out
}
pub fn add3n<'a>(v: &V3, k: f64, out: &'a mut V3) -> &'a mut V3 {
    out[0] = v[0] + k;
    out[1] = v[1] + k;
    out[2] = v[2] + k;
    out
}
pub fn sub3<'a>(v: &V3, w: &V3, out: &'a mut V3) -> &'a mut V3 {
    out[0] = v[0] - w[0];
    out[1] = v[1] - w[1];
    out[2] = v[2] - w[2];
    out
}
pub fn sub3n<'a>(v: &V3, k: f64, out: &'a mut V3) -> &'a mut V3 {
    out[0] = v[0] - k;
    out[1] = v[1] - k;
    out[2] = v[2] - k;
    out
}
pub fn mul3<'a>(v: &V3, w: &V3, out: &'a mut V3) -> &'a mut V3 {
    out[0] = v[0] * w[0];
    out[1] = v[1] * w[1];
    out[2] = v[2] * w[2];
    out
}
pub fn mul3n<'a>(v: &V3, k: f64, out: &'a mut V3) -> &'a mut V3 {
    out[0] = v[0] * k;
    out[1] = v[1] * k;
    out[2] = v[2] * k;
    out
}
pub fn div3<'a>(v: &V3, w: &V3, out: &'a mut V3) -> &'a mut V3 {
    out[0] = v[0] / w[0];
    out[1] = v[1] / w[1];
    out[2] = v[2] / w[2];
    out
}
pub fn div3n<'a>(v: &V3, k: f64, out: &'a mut V3) -> &'a mut V3 {
    out[0] = v[0] / k;
    out[1] = v[1] / k;
    out[2] = v[2] / k;
    out
}
pub fn mod3<'a>(v: &V3, w: &V3, out: &'a mut V3) -> &'a mut V3 {
    out[0] = v[0] % w[0];
    out[1] = v[1] % w[1];
    out[2] = v[2] % w[2];
    out
}
pub fn mod3n<'a>(v: &V3, k: f64, out: &'a mut V3) -> &'a mut V3 {
    out[0] = v[0] % k;
    out[1] = v[1] % k;
    out[2] = v[2] % k;
    out
}

pub fn dot3(v: &V3, w: &V3) -> f64 {
    v[0] * w[0] + v[1] * w[1] + v[2] * w[2]
}
pub fn len3(v: &V3) -> f64 {
    dot3(v, v).sqrt()
}
pub fn cross3<'a>(v: &V3, w: &V3, out: &'a mut V3) -> &'a mut V3 {
    out[0] = v[1] * w[2] - v[2] * w[1];
    out[1] = v[2] * w[0] - v[0] * w[2];
    out[2] = v[0] * w[1] - v[1] * w[0];
    out
}
pub fn norm3<'a>(v: &V3, out: &'a mut V3) -> &'a mut V3 {
    let len = match len3(v) {
        0.0 => 1.0,
        l => l,
    };
    out[0] = v[0] / len;
    out[1] = v[1] / len;
    out[2] = v[2] / len;
    out
}

use xpute_core::math::scalar::round;

pub fn floor3<'a>(base: &V3, span: &V3, out: &'a mut V3) -> &'a mut V3 {
    out[0] = (base[0] / span[0]).floor() * span[0];
    out[1] = (base[1] / span[1]).floor() * span[1];
    out[2] = (base[2] / span[2]).floor() * span[2];
    out
}
pub fn floor3n<'a>(v: &V3, k: f64, out: &'a mut V3) -> &'a mut V3 {
    out[0] = (v[0] / k).floor() * k;
    out[1] = (v[1] / k).floor() * k;
    out[2] = (v[2] / k).floor() * k;
    out
}
pub fn center3<'a>(base: &V3, span: &V3, out: &'a mut V3) -> &'a mut V3 {
    floor3(base, span, out);
    out[0] += span[0] * 0.5;
    out[1] += span[1] * 0.5;
    out[2] += span[2] * 0.5;
    out
}
pub fn center3n<'a>(v: &V3, k: f64, out: &'a mut V3) -> &'a mut V3 {
    floor3n(v, k, out);
    let half = k * 0.5;
    out[0] += half;
    out[1] += half;
    out[2] += half;
    out
}
pub fn ceil3<'a>(base: &V3, span: &V3, out: &'a mut V3) -> &'a mut V3 {
    out[0] = (base[0] / span[0]).ceil() * span[0];
    out[1] = (base[1] / span[1]).ceil() * span[1];
    out[2] = (base[2] / span[2]).ceil() * span[2];
    out
}
pub fn ceil3n<'a>(v: &V3, k: f64, out: &'a mut V3) -> &'a mut V3 {
    out[0] = (v[0] / k).ceil() * k;
    out[1] = (v[1] / k).ceil() * k;
    out[2] = (v[2] / k).ceil() * k;
    out
}
pub fn round3<'a>(base: &V3, span: &V3, out: &'a mut V3) -> &'a mut V3 {
    out[0] = round(base[0] / span[0]) * span[0];
    out[1] = round(base[1] / span[1]) * span[1];
    out[2] = round(base[2] / span[2]) * span[2];
    out
}
pub fn round3n<'a>(v: &V3, k: f64, out: &'a mut V3) -> &'a mut V3 {
    out[0] = round(v[0] / k) * k;
    out[1] = round(v[1] / k) * k;
    out[2] = round(v[2] / k) * k;
    out
}

/// `x | 0`: ToInt32 — the fraction dropped, modulo 2^32, NaN to 0.
fn to_int32(x: f64) -> i32 {
    if x.is_finite() {
        (x.trunc() % 4_294_967_296.0) as i64 as i32
    } else {
        0
    }
}

pub fn i3<'a>(v: &V3, out: &'a mut V3) -> &'a mut V3 {
    out[0] = to_int32(v[0]) as f64;
    out[1] = to_int32(v[1]) as f64;
    out[2] = to_int32(v[2]) as f64;
    out
}

pub fn snap3n<'a>(v: &V3, mask: i32, out: &'a mut V3) -> &'a mut V3 {
    out[0] = (to_int32(v[0]) & !mask) as f64;
    out[1] = (to_int32(v[1]) & !mask) as f64;
    out[2] = (to_int32(v[2]) & !mask) as f64;
    out
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

    pub fn nearest(self, p: V3) -> V3 {
        std::array::from_fn(|k| p[k].clamp(self.min[k], self.max[k]))
    }

    pub fn bounded(self) -> bool {
        self.min.iter().chain(&self.max).all(|c| c.is_finite())
    }
}
