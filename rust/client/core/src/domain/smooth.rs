// pxl-core/domain/smooth.rs

//! The one exponential smoothing that everything easing uses, the camera's
//! channels and the characters alike. Within `eps` of its target a value
//! takes the target whole, so the easing ends instead of asking for frames
//! forever over a remainder no screen shows.

use xpute_kit::math::scalar::wrap;

/// The share of the remaining distance covered in `delta` seconds at rate
/// `k`, `1 - e^(-k·delta)`. It traces the same curve at any frame rate.
pub(crate) fn smoothing_factor(k: f64, delta: f64) -> f64 {
    1.0 - libm::exp(-k * delta)
}

/// Eases an angle toward `target` the short way round. The result lies in
/// [-π, π).
pub(crate) fn smooth_angle(cur: f64, target: f64, k: f64, delta: f64, eps: f64) -> (f64, bool) {
    let pi = core::f64::consts::PI;
    let left = wrap(target - cur, -pi, pi);
    let ([moved], moving) = smooth_vec([0.0], [left], k, delta, eps);
    (wrap(cur + moved, -pi, pi), moving)
}

/// Eases every axis by the same share, so the path is a straight line. It
/// arrives once the remaining distance, over all axes together, is under
/// `eps`. The flag it returns says whether the value is still moving.
pub(crate) fn smooth_vec<const N: usize>(
    cur: [f64; N],
    target: [f64; N],
    k: f64,
    delta: f64,
    eps: f64,
) -> ([f64; N], bool) {
    let a = smoothing_factor(k, delta);
    let next: [f64; N] = std::array::from_fn(|i| cur[i] + (target[i] - cur[i]) * a);
    let left = (0..N)
        .map(|i| (target[i] - next[i]) * (target[i] - next[i]))
        .sum::<f64>()
        .sqrt();
    if left < eps {
        (target, false)
    } else {
        (next, true)
    }
}
