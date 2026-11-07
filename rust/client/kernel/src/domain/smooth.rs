// pxl-kernel/domain/smooth.rs

//! One exponential smoothing for everything that eases, the camera's
//! channels and the characters alike. Within `eps`
//! it takes the target whole, so it ends rather than asking for frames
//! forever on a remainder no screen shows.

use xpute_kit::math::scalar::wrap;

/// `1 - e^(-k·delta)`: the same curve at any frame rate.
pub fn smoothing_factor(k: f64, delta: f64) -> f64 {
    1.0 - libm::exp(-k * delta)
}

/// The short way round, in [-π, π).
pub fn smooth_angle(cur: f64, target: f64, k: f64, delta: f64, eps: f64) -> (f64, bool) {
    let pi = core::f64::consts::PI;
    let left = wrap(target - cur, -pi, pi);
    let ([moved], moving) = smooth_vec([0.0], [left], k, delta, eps);
    (wrap(cur + moved, -pi, pi), moving)
}

/// Every axis by the same share, so the path is straight; done within `eps`
/// in any direction.
pub fn smooth_vec<const N: usize>(
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
