// pxl-kernel/layer/environment/fog.rs

//! How far the view reaches, from how far the camera stands: the fog's band,
//! the clip planes and the sky, each render_cfg's value times one scale.
//!
//! The scale is 1 while the camera is near, so a view looking down a street
//! still has its horizon, and past the distance where FOG_PER_DIST overtakes
//! FOG_FAR it is the distance itself. From there on everything the view
//! measures grows with the zoom together, which keeps the ground a screen
//! shows the same few octaves at any zoom (terrain/level.rs `window`), and
//! the depth buffer's far-to-near ratio the same.

use crate::control::camera::config::camera_cfg;
use crate::layer::environment::config::render_cfg;

/// The factor the view's reach is scaled by with the camera `dist` from its
/// pivot.
pub fn view_scale(dist: f64) -> f64 {
    (render_cfg::FOG_PER_DIST * dist / render_cfg::FOG_FAR).max(1.0)
}

pub fn fog_near(dist: f64) -> f64 {
    render_cfg::FOG_NEAR * view_scale(dist)
}

/// Where the ground is hidden whole: how far it is seen.
pub fn fog_far(dist: f64) -> f64 {
    render_cfg::FOG_FAR * view_scale(dist)
}

pub fn near_clip(dist: f64) -> f64 {
    camera_cfg::projection::NEAR * view_scale(dist)
}

pub fn far_clip(dist: f64) -> f64 {
    render_cfg::FAR_CLIP * view_scale(dist)
}

/// The sky is the inside of a sphere centred on the world origin, and must
/// stay inside the far clip plane. The camera can stand as far from that
/// origin as it stands from its pivot, and looking toward the far side of
/// the sphere from an off-centre point adds that same distance again, so the
/// sphere leaves it as margin; without it the sky clips exactly where a
/// wide-shot camera is most likely to be looking. The camera distance the
/// margin is taken at is the one the scale holds the far clip at.
pub fn sky_radius(dist: f64) -> f64 {
    (render_cfg::FAR_CLIP - render_cfg::FOG_FAR / render_cfg::FOG_PER_DIST) * 0.9 * view_scale(dist)
}

#[cfg(test)]
#[path = "fog.test.rs"]
mod test;
