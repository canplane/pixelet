// pxl-kernel/layer/environment/fog.rs

//! How far the view reaches, and the clip planes set from it. The nearer of
//! two ends wins: the horizon, `√(2Rh + h²)`, and `VIEW_DOUBLINGS` doublings
//! past the ground under the eye, which bounds the zooms drawn. The fog marks
//! where drawing stops, not the air.

use pxl_spec::space::lod::split_distance;
use pxl_spec::space::pvf3::projection;
use pxl_spec::space::pyramid::GROUND_Z;

use crate::control::camera::config::camera_cfg;
use crate::layer::environment::config::render_cfg;
use crate::layer::environment::state::globe;
use crate::param;
use crate::world::access::Access;
use crate::world::observation::eye;

pub fn horizon_at(height: f64) -> f64 {
    let r = globe().radius_u();
    (2.0 * r * height + height * height).sqrt()
}

/// The nearer of height and z16's split distance, doubled VIEW_DOUBLINGS times.
pub fn doublings_reach_at(height: f64) -> f64 {
    height.max(split_distance(GROUND_Z as i32)) * f64::from(1u32 << param::VIEW_DOUBLINGS)
}

/// On the ground the geometric horizon is underfoot and the view would hold nothing.
const NEAREST_REACH: f64 = 1024.0;

pub fn reach_at(height: f64) -> f64 {
    horizon_at(height).min(doublings_reach_at(height)).max(NEAREST_REACH)
}

/// From the point's own height, so a cave's floor counts in a cave; the
/// datum where nothing is held.
fn ground_under(at: [f64; 3]) -> f64 {
    let point = projection::project(at[0], at[1], at[2]);
    Access::now().floor_under(&point).value().flatten().unwrap_or(0.0)
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

/// Fade start and end. Where the data runs out before the reach, the band
/// crossfades over the edge of what is drawn.
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
