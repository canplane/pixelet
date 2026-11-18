// pxl-core/domain/smooth.rs

//! The one exponential smoothing that everything easing uses. It nears its
//! target and never arrives: whoever eases ends it, by what the eye can
//! tell apart.

use xpute_kit::math::scalar::wrap;

/// The share of the remaining distance covered in `delta` seconds at rate
/// `k`, `1 - e^(-k·delta)`. It traces the same curve at any frame rate.
pub(crate) fn smoothing_factor(k: f64, delta: f64) -> f64 {
    1.0 - libm::exp(-k * delta)
}

/// Eases an angle toward `target` the short way round. The result lies in
/// [-π, π).
pub(crate) fn smooth_angle(cur: f64, target: f64, k: f64, delta: f64) -> f64 {
    let pi = core::f64::consts::PI;
    let left = wrap(target - cur, -pi, pi);
    let [moved] = smooth_vec([0.0], [left], k, delta);
    wrap(cur + moved, -pi, pi)
}

/// Eases every axis by the same share, so the path is a straight line.
pub(crate) fn smooth_vec<const N: usize>(
    cur: [f64; N],
    target: [f64; N],
    k: f64,
    delta: f64,
) -> [f64; N] {
    let a = smoothing_factor(k, delta);
    std::array::from_fn(|i| cur[i] + (target[i] - cur[i]) * a)
}
