// xpute_core/kit/math.rs

pub const EPS: f64 = 1e-5;

/// Precision equality check using both absolute and relative tolerance.
/// As magnitudes grow, the allowed error scales proportionally with them.
pub fn approx_eq(a: f64, b: f64, eps: Option<f64>) -> bool {
    let eps = eps.unwrap_or(EPS);
    if a == b {
        return true;
    }
    let diff = (a - b).abs();
    // absolute tolerance (small values) || relative tolerance (large values)
    diff < eps || diff < eps * a.abs().max(b.abs())
}

/// Physical/intuitive proximity check using only absolute distance.
pub fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() < tol
}

pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}
pub fn inv_lerp(x: f64, min: f64, max: f64) -> f64 {
    (x - min) / (max - min)
}

/// Remaps a value from one range into another.
pub fn remap(x: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> f64 {
    lerp(out_min, out_max, inv_lerp(x, in_min, in_max))
}

pub fn clamp(x: f64, min: f64, max: f64) -> f64 {
    if x < min {
        min
    } else if x > max {
        max
    } else {
        x
    }
}
pub fn wrap(x: f64, min: f64, max: f64) -> f64 {
    let range = max - min;
    ((((x - min) % range) + range) % range) + min
}

pub fn floor_to_step(x: f64, step: f64) -> f64 {
    (x / step).floor() * step
}
pub fn round_to_step(x: f64, step: f64) -> f64 {
    crate::kit::fdlibm::round(x / step) * step
}
pub fn ceil_to_step(x: f64, step: f64) -> f64 {
    (x / step).ceil() * step
}

#[cfg(test)]
#[path = "math.test.rs"]
mod test;
