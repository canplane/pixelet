// pxl-kernel/control/camera/approach.rs

//! How a camera's drawn state comes to what its controls ask: one law, an
//! exponential approach at a rate, for every channel a camera component
//! moves. The component knows its own channels — the orbit its map center,
//! radius and look, the free camera its eye and look — and its own target
//! and current state of each; what is here knows none of them, only a value,
//! where it is going, how fast, and how close is close enough to stop.
//!
//! The view runs a frame only when something asks (sched/tick.rs), and an
//! approach asks until it is done: within `eps` of its target it takes the
//! target whole, so it ends rather than closing forever on a remainder no
//! screen shows.

use xpute_core::math::scalar::wrap;

/// The share of what is left that `delta` seconds at rate `k` close: the
/// same curve whatever the frame rate, `1 - e^(-k·delta)`.
pub fn damp_alpha(k: f64, delta: f64) -> f64 {
    1.0 - libm::exp(-k * delta)
}

/// `current` moved toward `target` over `delta` seconds at rate `k`, and
/// whether it is still on its way.
pub fn approach_scalar(current: f64, target: f64, k: f64, delta: f64, eps: f64) -> (f64, bool) {
    let next = current + (target - current) * damp_alpha(k, delta);
    if (target - next).abs() < eps {
        (target, false)
    } else {
        (next, true)
    }
}

/// An angle moved toward another the short way round, in a turn's domain
/// [-π, π).
pub fn approach_angle(current: f64, target: f64, k: f64, delta: f64, eps: f64) -> (f64, bool) {
    let pi = core::f64::consts::PI;
    let left = wrap(target - current, -pi, pi);
    let (moved, moving) = approach_scalar(0.0, left, k, delta, eps);
    (wrap(current + moved, -pi, pi), moving)
}

/// A point moved toward another along the line between them, every axis by
/// the same law, so the path is straight and isotropic: what is left on
/// each axis shrinks by one share. Done once what is left is within `eps`
/// whatever its direction.
pub fn approach_vec<const N: usize>(current: [f64; N], target: [f64; N], k: f64, delta: f64, eps: f64) -> ([f64; N], bool) {
    let a = damp_alpha(k, delta);
    let next: [f64; N] = std::array::from_fn(|i| current[i] + (target[i] - current[i]) * a);
    let left = (0..N).map(|i| (target[i] - next[i]).powi(2)).sum::<f64>().sqrt();
    if left < eps {
        (target, false)
    } else {
        (next, true)
    }
}
