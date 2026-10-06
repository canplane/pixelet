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

pub fn floor3(v: &V3, k: f64) -> V3 {
    [(v[0] / k).floor() * k, (v[1] / k).floor() * k, (v[2] / k).floor() * k]
}
pub fn ceil3(v: &V3, k: f64) -> V3 {
    [(v[0] / k).ceil() * k, (v[1] / k).ceil() * k, (v[2] / k).ceil() * k]
}
/// The middle of the cell of side `k` holding `v`.
pub fn center3(v: &V3, k: f64) -> V3 {
    floor3(v, k).map(|c| c + k * 0.5)
}
