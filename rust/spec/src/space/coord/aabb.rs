// pxl-spec/space/coord/aabb.rs

//! A box of the world: two corners in units, its sides along the axes.

use super::v3::{self, V3};

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
        (0..3)
            .all(|k| min[k] <= max[k])
            .then_some(Aabb { min, max })
    }

    /// Whether the two share room, not only a face.
    pub fn shares(self, other: Aabb) -> bool {
        (0..3).all(|k| self.min[k] < other.max[k] && other.min[k] < self.max[k])
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
        v3::len(&self.extent()) / 2.0
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

    pub fn nearest(self, p: V3) -> V3 {
        std::array::from_fn(|k| p[k].clamp(self.min[k], self.max[k]))
    }

    pub fn bounded(self) -> bool {
        self.min.iter().chain(&self.max).all(|c| c.is_finite())
    }

    /// Where a ray from `origin` along `direction` enters it, at `t`
    /// along it, and the axis it enters across: of its near planes, the last
    /// the ray reaches. None where the ray misses it, or starts inside it.
    pub fn enters(self, origin: V3, direction: V3) -> Option<(f64, usize)> {
        let (mut near, mut far, mut axis) = (f64::NEG_INFINITY, f64::INFINITY, 2);
        for k in 0..3 {
            if direction[k] == 0.0 {
                if origin[k] < self.min[k] || self.max[k] < origin[k] {
                    return None;
                }
                continue;
            }
            let [a, b] = [self.min[k], self.max[k]].map(|p| (p - origin[k]) / direction[k]);
            let (lo, hi) = if a < b { (a, b) } else { (b, a) };
            if lo > near {
                (near, axis) = (lo, k);
            }
            far = far.min(hi);
        }
        (0.0 <= near && near <= far).then_some((near, axis))
    }
}

#[cfg(test)]
#[path = "aabb.test.rs"]
mod test;
