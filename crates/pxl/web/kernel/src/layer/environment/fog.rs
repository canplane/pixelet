// pxl-kernel/layer/environment/fog.rs

//! How far the view reaches, and the clip planes set from it.
//!
//! The ground ends twice and the nearer end wins. The planet turns it away at
//! the **horizon**, `√(2Rh + h²)` from a camera `h` above it, which the height
//! alone sets and the tilt does not. And the view stops drawing it:
//! `param::VIEW_LEVELS` doublings of distance past the ground under the eye,
//! or z16's split distance where that is nearer, a level a doubling — so a
//! view draws at most that many levels and one at any height, and what it
//! costs is held by that count rather than by how far the planet lets it
//! see. The fog is laid over the second
//! end as a sphere about the eye: it says where drawing stops, and does not
//! stand for the air (issue/20260923-the-ground-runs-to-the-horizon.md says
//! how the reach was the air's before).

use pxl_spec::space::lod::split_distance;
use pxl_spec::space::pvf3::projection;

use crate::control::camera::config::camera_cfg;
use crate::layer::environment::config::render_cfg;
use crate::layer::environment::state::globe;
use crate::param;
use crate::world::access::{Access, FINEST};
use crate::world::observation::eye;

/// Where the planet turns away, seen from `height` above it.
pub fn horizon_at(height: f64) -> f64 {
    let r = globe().radius_u();
    (2.0 * r * height + height * height).sqrt()
}

/// Where the view stops drawing, seen from `height` over the ground:
/// `VIEW_LEVELS` doublings past the nearer of the ground under the eye and
/// z16's split distance, where the finest level ends.
pub fn levels_reach_at(height: f64) -> f64 {
    height.max(split_distance(0)) * f64::from(1u32 << param::VIEW_LEVELS)
}

/// On the ground the geometric horizon is underfoot and the view would hold nothing.
const NEAREST_REACH: f64 = 1024.0;

/// How far the ground is drawn from `height` above it.
pub fn reach_at(height: f64) -> f64 {
    horizon_at(height).min(levels_reach_at(height)).max(NEAREST_REACH)
}

/// From the point's own height, so a cave's floor counts in a cave; the
/// datum where nothing is held.
fn ground_under(at: [f64; 3]) -> f64 {
    let point = projection::project(at[0], at[1], at[2]);
    Access::now().floor_under(&point).value().unwrap_or(0.0)
}

/// Where the sky draws its own ground, so the terrain does not stand over it.
pub fn eye_ground() -> f64 {
    eye().map_or(0.0, ground_under)
}

/// What the reach, fog band, clip planes and origin grain are measured from.
pub fn height_at(at: [f64; 3]) -> f64 {
    (at[2] - ground_under(at)).max(0.0)
}

pub fn eye_height() -> f64 {
    eye().map_or(0.0, height_at)
}

pub fn view_reach() -> f64 {
    reach_at(eye_height())
}

/// Never under 1, so a street view keeps its precision and a zoom out is only
/// a change of scale.
fn scale(reach: f64) -> f64 {
    (reach / render_cfg::FOG_FAR).max(1.0)
}

/// Where the ground is gone, and where it begins to fade: the band that
/// hides the drawn edge.
///
/// The ground ends at whichever comes first, the reach or what is drawn,
/// which runs to the pyramid top's reach (world/observation.rs
/// `observation`): where the data runs out before the reach does, the band
/// is a crossfade over the edge of what is drawn rather than over the reach.
pub fn band_at(height: f64, drawn: f64) -> (f64, f64) {
    let far = reach_at(height).min(drawn);
    (far * (render_cfg::FOG_NEAR / render_cfg::FOG_FAR), far)
}

pub fn near_clip_at(height: f64) -> f64 {
    camera_cfg::projection::NEAR * scale(reach_at(height))
}

/// Past the reach, so the band completes inside the frustum.
pub fn far_clip_at(height: f64) -> f64 {
    reach_at(height) * (render_cfg::FAR_CLIP / render_cfg::FOG_FAR)
}
