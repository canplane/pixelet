// xpute-core/math/scalar.rs

pub const EPS: f64 = 1e-5;

/// Absolute tolerance near zero, relative beyond.
pub fn approx_eq(a: f64, b: f64, eps: Option<f64>) -> bool {
    let eps = eps.unwrap_or(EPS);
    if a == b {
        return true;
    }
    let diff = (a - b).abs();
    diff < eps || diff < eps * a.abs().max(b.abs())
}

pub fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() < tol
}

/// JavaScript's `Math.round`: `-2.5` is `-2`.
pub fn round_half_up(x: f64) -> f64 {
    if x.fract() == -0.5 {
        x.ceil()
    } else {
        x.round()
    }
}

/// C's and Rust's `round`: `-2.5` is `-3`.
pub fn round_half_away(x: f64) -> f64 {
    x.round()
}

/// IEEE 754's and WGSL's: `2.5` is `2`.
pub fn round_half_even(x: f64) -> f64 {
    x.round_ties_even()
}

/// To even, as WGSL's `round`, so the CPU and a shader land on one integer.
/// `f64::round` is not this.
pub use self::round_half_even as round;

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
    round(x / step) * step
}
pub fn ceil_to_step(x: f64, step: f64) -> f64 {
    (x / step).ceil() * step
}

#[cfg(test)]
#[path = "scalar.test.rs"]
mod test;
