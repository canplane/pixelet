// xpute-core/math/scalar.rs

use core::ops::{Add, Div, Mul, Neg, Rem, Sub};

pub const EPS: f64 = 1e-5;

/// What these functions take: `f64`, and `f32` where a value lives as one.
pub trait Float: Copy + PartialOrd + Add<Output = Self> + Sub<Output = Self> + Mul<Output = Self> + Div<Output = Self> + Rem<Output = Self> + Neg<Output = Self> {
    const EPS: Self;
    const HALF: Self;
    fn abs(self) -> Self;
    fn max(self, other: Self) -> Self;
    fn floor(self) -> Self;
    fn ceil(self) -> Self;
    fn fract(self) -> Self;
    fn round_away(self) -> Self;
    fn round_even(self) -> Self;
}

macro_rules! float {
    ($t:ty) => {
        impl Float for $t {
            const EPS: $t = EPS as $t;
            const HALF: $t = 0.5;
            fn abs(self) -> $t {
                <$t>::abs(self)
            }
            fn max(self, other: $t) -> $t {
                <$t>::max(self, other)
            }
            fn floor(self) -> $t {
                <$t>::floor(self)
            }
            fn ceil(self) -> $t {
                <$t>::ceil(self)
            }
            fn fract(self) -> $t {
                <$t>::fract(self)
            }
            fn round_away(self) -> $t {
                <$t>::round(self)
            }
            fn round_even(self) -> $t {
                <$t>::round_ties_even(self)
            }
        }
    };
}

float!(f32);
float!(f64);

/// Absolute tolerance near zero, relative beyond.
pub fn approx_eq<T: Float>(a: T, b: T, eps: Option<T>) -> bool {
    let eps = eps.unwrap_or(T::EPS);
    if a == b {
        return true;
    }
    let diff = (a - b).abs();
    diff < eps || diff < eps * a.abs().max(b.abs())
}

pub fn near<T: Float>(a: T, b: T, tol: T) -> bool {
    (a - b).abs() < tol
}

/// JavaScript's `Math.round`: `-2.5` is `-2`.
pub fn round_half_up<T: Float>(x: T) -> T {
    if x.fract() == -T::HALF {
        x.ceil()
    } else {
        x.round_away()
    }
}

/// C's and Rust's `round`: `-2.5` is `-3`.
pub fn round_half_away<T: Float>(x: T) -> T {
    x.round_away()
}

/// IEEE 754's and WGSL's: `2.5` is `2`.
pub fn round_half_even<T: Float>(x: T) -> T {
    x.round_even()
}

/// To even, as WGSL's `round`, so the CPU and a shader land on one integer.
/// `f64::round` is not this.
pub use self::round_half_even as round;

pub fn lerp<T: Float>(a: T, b: T, t: T) -> T {
    a + (b - a) * t
}
pub fn inv_lerp<T: Float>(x: T, min: T, max: T) -> T {
    (x - min) / (max - min)
}

/// Remaps a value from one range into another.
pub fn remap<T: Float>(x: T, in_min: T, in_max: T, out_min: T, out_max: T) -> T {
    lerp(out_min, out_max, inv_lerp(x, in_min, in_max))
}

pub fn clamp<T: Float>(x: T, min: T, max: T) -> T {
    if x < min {
        min
    } else if x > max {
        max
    } else {
        x
    }
}
pub fn wrap<T: Float>(x: T, min: T, max: T) -> T {
    let range = max - min;
    ((((x - min) % range) + range) % range) + min
}

pub fn floor_to_step<T: Float>(x: T, step: T) -> T {
    (x / step).floor() * step
}
pub fn round_to_step<T: Float>(x: T, step: T) -> T {
    round(x / step) * step
}
pub fn ceil_to_step<T: Float>(x: T, step: T) -> T {
    (x / step).ceil() * step
}

#[cfg(test)]
#[path = "scalar.test.rs"]
mod test;
