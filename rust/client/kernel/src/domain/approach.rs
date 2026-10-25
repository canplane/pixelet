// pxl-kernel/domain/approach.rs

//! One exponential approach for everything that eases, the camera's
//! channels and the companion alike. Within `eps`
//! it takes the target whole, so it ends rather than asking for frames
//! forever on a remainder no screen shows.

use xpute_kit::math::scalar::wrap;

/// `1 - e^(-k·delta)`: the same curve at any frame rate.
pub fn damp_alpha(k: f64, delta: f64) -> f64 {
    1.0 - libm::exp(-k * delta)
}

/// The short way round, in [-π, π).
pub fn approach_angle(current: f64, target: f64, k: f64, delta: f64, eps: f64) -> (f64, bool) {
    let pi = core::f64::consts::PI;
    let left = wrap(target - current, -pi, pi);
    let ([moved], moving) = approach_vec([0.0], [left], k, delta, eps);
    (wrap(current + moved, -pi, pi), moving)
}

/// Every axis by the same share, so the path is straight; done within `eps`
/// in any direction.
pub fn approach_vec<const N: usize>(
    current: [f64; N],
    target: [f64; N],
    k: f64,
    delta: f64,
    eps: f64,
) -> ([f64; N], bool) {
    let a = damp_alpha(k, delta);
    let next: [f64; N] = std::array::from_fn(|i| current[i] + (target[i] - current[i]) * a);
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
