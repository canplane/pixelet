// pxl-kernel/control/camera/orbit.rs

//! The orbit camera: one controller of the observer's pose. Its controls —
//! a pivot, a distance, an angle down from vertical and a heading about Z —
//! are damped toward the camera store's (camera/damping.rs,
//! world/frame/view_frame.rs), and the pose is made from them here
//! (`orbit_pose`): the eye at the distance from the pivot, looking at it.
//! The world stays in its own axes; what turns is the observer. Nothing
//! below the pose knows it came from an orbit.

use pxl_spec::space::mat4::{mat4, mat4_look_at, mat4_mul, mat4_rotation_z, Mat4};
use pxl_spec::space::v3::V3;

use crate::control::camera::config::camera_cfg;
use crate::global::Global;
use crate::world::frame::view::ObserverPose;

/// The orbit as drawn: its controls as their damping steps have eased them.
pub struct OrbitView {
    /// The eye's distance from the pivot and its angle down from vertical,
    /// the climb included.
    pub dist: f64,
    pub polar: f64,
    /// The heading about Z: at zero the eye stands south of the pivot and
    /// looks north.
    pub azimuth: f64,
    /// What the eye looks at: the map center as the view has eased toward
    /// it, in the view's root frame (world/frame/view.rs).
    pub pivot: V3,
}

static ORBIT: Global<OrbitView> = Global::new();

pub fn orbit_view() -> &'static mut OrbitView {
    ORBIT.get(|| OrbitView {
        dist: camera_cfg::view::DEFAULT_DIST,
        polar: 0.0,
        azimuth: 0.0,
        pivot: [0.0; 3],
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitOffset {
    pub horiz_dist: f64,
    pub vert_dist: f64,
}

/// How far the eye stands from the pivot across and up, given the
/// (already-damped/clamped) dist/polar. `polar` is measured from the +Z
/// (vertical) axis: polar≈0 looks straight down, polar≈π/2 looks toward the
/// horizon.
pub fn orbit_local_offset(dist: f64, polar: f64) -> OrbitOffset {
    OrbitOffset {
        horiz_dist: dist * libm::sin(polar),
        vert_dist: dist * libm::cos(polar),
    }
}

/// The eye's offset from the pivot, in the world's axes: south of it at a
/// heading of zero, turned about Z by the heading.
pub fn orbit_eye_offset(dist: f64, polar: f64, azimuth: f64) -> V3 {
    let o = orbit_local_offset(dist, polar);
    let (s, c) = (libm::sin(azimuth), libm::cos(azimuth));
    [-o.horiz_dist * s, -o.horiz_dist * c, o.vert_dist]
}

/// The observer's pose the orbit makes: the eye at its offset from the
/// pivot, and the turn from the world's axes to the camera's that looks
/// from there at the pivot with Z up.
pub fn orbit_pose(o: &OrbitView) -> ObserverPose {
    let off = orbit_local_offset(o.dist, o.polar);
    // Looking from south of the pivot, then the world's axes turned into
    // that by the heading.
    let mut look = mat4();
    mat4_look_at(&mut look, [0.0, -off.horiz_dist, off.vert_dist], [0.0; 3], [0.0, 0.0, 1.0]);
    look[12..15].fill(0.0);
    let mut heading = mat4();
    mat4_rotation_z(&mut heading, o.azimuth);
    let mut rotation: Mat4 = mat4();
    mat4_mul(&mut rotation, &look, &heading);
    let e = orbit_eye_offset(o.dist, o.polar, o.azimuth);
    ObserverPose {
        eye: [o.pivot[0] + e[0], o.pivot[1] + e[1], o.pivot[2] + e[2]],
        rotation,
    }
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
