// pxl-kernel/layer/environment/fog.rs

//! How far the view reaches, from how far the camera stands: the fog's band
//! and the clip planes, each render_cfg's value times one scale.
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

#[cfg(test)]
#[path = "fog.test.rs"]
mod test;
