// pxl_shared/spec/space/v3.rs

//! The TypeScript's optional `out` parameter, defaulting to a new vector,
//! is a required one here: every function writes into the `out` it is
//! given and returns it.

use xpute_core::kit::fdlibm;

/// Fixed-length 3D float vector.
pub type V3 = [f64; 3];

/// Fills all three lanes with the same scalar.
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

// ---- Arithmetic ----

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
pub fn cross3<'a>(v: &V3, w: &V3, out: &'a mut V3) -> &'a mut V3 {
    out[0] = v[1] * w[2] - v[2] * w[1];
    out[1] = v[2] * w[0] - v[0] * w[2];
    out[2] = v[0] * w[1] - v[1] * w[0];
    out
}
pub fn norm3<'a>(v: &V3, out: &'a mut V3) -> &'a mut V3 {
    let hypot = fdlibm::hypot(&[v[0], v[1], v[2]]);
    let len = if hypot == 0.0 { 1.0 } else { hypot };
    out[0] = v[0] / len;
    out[1] = v[1] / len;
    out[2] = v[2] / len;
    out
}

use xpute_core::kit::fdlibm::round;

/// Floors each lane to the lower multiple of the corresponding span.
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
/// Returns the snapped cell center for `base` under `span`.
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

#[cfg(test)]
#[path = "v3.test.rs"]
mod test;
