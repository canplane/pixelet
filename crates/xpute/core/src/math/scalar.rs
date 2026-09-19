// xpute-core/math/scalar.rs

//! Functions of one double: comparison within a tolerance, the three ways a
//! half rounds and the one the project takes, interpolation, and clamping or
//! wrapping into a range.

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

/// The three ways a half rounds, each by the name of where it goes. The
/// project rounds to even — IEEE 754's default, and what WGSL's `round`
/// does — so a value the CPU and a shader both round lands on one integer;
/// the other two are here for the places that have to agree with a
/// language that chose otherwise.
///
/// Toward +∞: JavaScript's `Math.round`, Java's. `-2.5` is `-2`.
pub fn round_half_up(x: f64) -> f64 {
    if x.fract() == -0.5 {
        x.ceil()
    } else {
        x.round()
    }
}

/// Away from zero: C's `round`, Rust's. `-2.5` is `-3`.
pub fn round_half_away(x: f64) -> f64 {
    x.round()
}

/// To even: IEEE 754's, WGSL's, Python's. `2.5` is `2`, `-2.5` is `-2`.
pub fn round_half_even(x: f64) -> f64 {
    x.round_ties_even()
}

/// The project's rounding, which is to even. One line to change if it ever
/// is not; `f64::round` is not it and does not appear in the project.
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
