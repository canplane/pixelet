// pxl-kernel/layer/environment/fog.rs

//! How far the view reaches, and the clip planes set from it.
//!
//! On a planet the ground ends twice and the nearer end wins. It turns away
//! at the **horizon**, `√(2Rh + h²)` from a camera `h` above it, which the
//! height alone sets and the tilt does not. Before that it loses its contrast
//! in the air: the aerosol near the ground (pxl-render look/atmosphere.rs)
//! takes a surface down to the sky's own light over the **visual range**, and
//! past that there is nothing left to draw.
//!
//! Which of the two is nearer changes with height, and both move out with
//! it. Standing on the ground the horizon is a few kilometres and the air is
//! clear well past it, so the planet ends the ground; from a few hundred
//! metres up the horizon has run past the visual range and the haze ends it;
//! from orbit the layer is one crossing however far the eye looks, the air
//! sees through it, and the planet ends the ground again.
//!
//! What stood here before was a render distance: a constant scaled with the
//! zoom, with the band that hid the drawn edge following it. Both are now
//! read off the reach, so the ground ends where the world says rather than
//! where a number was tuned
//! (issue/20260923-the-ground-runs-to-the-horizon.md).

use pxl_render::look::atmosphere::{HAZE_EXTINCTION, HAZE_SCALE_HEIGHT_KM};
use pxl_spec::space::pvf3::{fvec, ivec};

use crate::control::camera::config::camera_cfg;
use crate::layer::environment::config::render_cfg;
use crate::layer::environment::state::{globe, km_per_u};
use crate::layer::terrain::elevation::get_ele;
use crate::shell::map_store::map_store;
use crate::world::frame::view::view;

/// The optical depth at which a surface's contrast against the horizon sky is
/// gone: `ln(1/0.02)`, the 2% the visual range is defined by (Koschmieder).
const CONTRAST_GONE: f64 = 3.912;

/// Where the planet turns away, seen from `height` above it.
pub fn horizon_at(height: f64) -> f64 {
    let r = globe().radius_u();
    (2.0 * r * height + height * height).sqrt()
}

/// Where the haze has taken the ground, seen from `height` above it: the
/// distance whose slant through the aerosol carries CONTRAST_GONE.
///
/// The aerosol is mixed through a layer HAZE_SCALE_HEIGHT_KM deep, so a ray
/// from `h` down to the ground averages `H(1 − e^(−h/H))/h` of the surface
/// density over its length — the layer's integral along a straight line. A
/// camera in the layer sees the whole density and a camera above it crosses
/// the layer once however far it looks, which is why this grows with height
/// until the horizon overtakes it.
pub fn haze_reach_at(height: f64) -> f64 {
    let km_per_u = km_per_u();
    let h = (height * km_per_u).max(1e-9);
    let scale = HAZE_SCALE_HEIGHT_KM;
    let share = scale * (1.0 - (-h / scale).exp()) / h;
    CONTRAST_GONE / (HAZE_EXTINCTION * share) / km_per_u
}

/// Nothing is drawn nearer than this, whatever the two ends say: the camera
/// can stand on the ground, where the geometric horizon is under its own
/// feet and a view along it would hold nothing at all.
const NEAREST_REACH: f64 = 1024.0;

/// How far the ground is worth drawing from `height` above it.
pub fn reach_at(height: f64) -> f64 {
    horizon_at(height).min(haze_reach_at(height)).max(NEAREST_REACH)
}

/// How high the ground under the map centre stands over the datum the
/// projection measures from: where the sky draws its own ground, so the
/// terrain, which is drawn at its elevation, does not stand over it
/// (pxl-render atmos_medium.wgsl's land_altitude_km).
pub fn centre_ground() -> f64 {
    map_store().map_center.and_then(|c| get_ele(&c)).unwrap_or(0.0)
}

/// How high the map centre stands over the ground under it: on a building,
/// that much further below the camera.
pub fn pivot_height() -> f64 {
    map_store().map_center.map_or(0.0, |c| {
        let z = ivec::to_real(ivec::Z(c.i)) + fvec::to_real(fvec::Z(c.f));
        get_ele(&c).map_or(0.0, |ground| (z - ground).max(0.0))
    })
}

/// The camera's height over the ground, for a camera `dist` from its pivot at
/// `polar` from straight down.
pub fn camera_height(dist: f64, polar: f64) -> f64 {
    pivot_height() + dist * polar.cos()
}

/// The reach of the view as it is drawn.
pub fn view_reach() -> f64 {
    let v = view();
    reach_at(camera_height(v.dist, v.polar))
}

/// What the near plane and the sizes tuned with it are scaled by: the reach
/// against the one they were tuned at, never under 1, so a street view keeps
/// the precision it had and a zoom out is a change of scale and nothing else.
fn scale(reach: f64) -> f64 {
    (reach / render_cfg::FOG_FAR).max(1.0)
}

/// Where the ground is gone, and where it begins to fade: the band that
/// hides the drawn edge.
///
/// The ground ends at whichever comes first, the air and the planet or the
/// rings themselves. The window holds the levels `.fixed` is sized for and
/// no more (world/pyramid/window.rs), so from a few hundred metres up the data runs
/// out before the air does, and the band is then a crossfade over the edge
/// of what is drawn rather than over the reach.
pub fn band_at(height: f64, drawn: f64) -> (f64, f64) {
    let far = reach_at(height).min(drawn);
    (far * (render_cfg::FOG_NEAR / render_cfg::FOG_FAR), far)
}

pub fn near_clip_at(height: f64) -> f64 {
    camera_cfg::projection::NEAR * scale(reach_at(height))
}

/// Past the reach, so the band completes inside the frustum rather than
/// being cut by it.
pub fn far_clip_at(height: f64) -> f64 {
    reach_at(height) * (render_cfg::FAR_CLIP / render_cfg::FOG_FAR)
}

#[cfg(test)]
#[path = "fog.test.rs"]
mod test;
