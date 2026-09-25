// pxl-kernel/control/camera/orbit.rs

//! Pure math for this app's orbit camera model, pulled out of damping.rs /
//! view_frame.rs so it can be unit tested on its own. Doc'd once here
//! because the model is easy to get backwards:
//!
//! **Azimuth never rotates the camera.** Only `dist` and `polar` determine
//! camera.position (orbit_local_offset below), and the camera always looks
//! straight at its target. World content rotates by azimuth under this fixed
//! camera instead (world/frame/view.rs, stepped by view_frame.rs) — the
//! ground spins, not the eye. Anything needing "which way is the camera
//! effectively facing" — the sky dome's sun and moon sweep — has
//! no single camera transform to read that off of — only this rotate2
//! primitive, signed for the conversion direction: world_frame_ref's own
//! rotation.z is always exactly +azimuth, so matching the ground's own
//! transform means rotate2(..., +azimuth), and un-rotating a camera-local
//! point back to its ground origin means rotate2(..., -azimuth).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitOffset {
    pub horiz_dist: f64,
    pub vert_dist: f64,
}

/// The camera's local-space offset from its look-at target, given the current
/// (already-damped/clamped) dist/polar. `polar` is measured from the +Z
/// (vertical) axis: polar≈0 looks straight down, polar≈π/2 looks toward the
/// horizon. The camera sits on the target's local -Y side — an arbitrary but
/// fixed choice, since azimuth (not this offset) is what actually varies the
/// apparent viewing direction, by spinning the world underneath it instead.
pub fn orbit_local_offset(dist: f64, polar: f64) -> OrbitOffset {
    OrbitOffset {
        horiz_dist: dist * libm::sin(polar),
        vert_dist: dist * libm::cos(polar),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rotated {
    pub x: f64,
    pub y: f64,
}

/// Rotates a 2D vector by `angle` around the Z axis (scene is Z-up):
/// (x cosθ - y sinθ, x sinθ + y cosθ). The one shared primitive behind every
/// azimuth-aware transform in this module tree. Callers pick the sign of
/// `angle` for their own direction of conversion — see this module's doc
/// comment above.
pub fn rotate2(x: f64, y: f64, angle: f64) -> Rotated {
    let c = libm::cos(angle);
    let s = libm::sin(angle);
    Rotated { x: x * c - y * s, y: x * s + y * c }
}

/// Exponential-decay ease-toward-target factor for one frame of `delta`
/// seconds at rate `k` (bigger k = snappier convergence). Shared by every
/// damped camera/frame value in this module tree (dist/polar in
/// damping.rs, azimuth in view_frame.rs) so they all converge with
/// the same characteristic curve — apply as
/// `current += (target - current) * damp_alpha(k, delta)`.
pub fn damp_alpha(k: f64, delta: f64) -> f64 {
    1.0 - libm::exp(-k * delta)
}
