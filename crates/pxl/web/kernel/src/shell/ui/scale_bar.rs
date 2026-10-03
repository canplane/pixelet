// pxl-kernel/shell/ui/scale_bar.rs

//! What the view's scale reads as: the scale bar (at most
//! SCALE_BAR_MAX_WIDTH_PX, rounded to 1/2/5 steps) and the equivalent Web
//! Mercator zoom.

use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::pvf3;

use crate::control::camera::config::camera_cfg;

pub const SCALE_BAR_MAX_WIDTH_PX: f64 = 80.0;

pub fn round_to_nice_scale(raw: f64) -> f64 {
    if raw.is_nan() || raw <= 0.0 {
        return 0.0;
    }
    let exponent = raw.log10().floor();
    let base = 10f64.powf(exponent);
    let fraction = raw / base;
    let nice_fraction = if fraction < 1.5 {
        1.0
    } else if fraction < 3.5 {
        2.0
    } else if fraction < 7.5 {
        5.0
    } else {
        10.0
    };
    nice_fraction * base
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScaleBar {
    pub total_meters: f64,
    pub bar_width_px: f64,
}

/// u a pixel spans at the camera's distance.
fn u_per_px(screen_height: f64) -> f64 {
    let dist = crate::control::camera::controller::radius();
    let fov_rad = camera_cfg::projection::FOV.to_radians();
    2.0 * (fov_rad / 2.0).tan() * dist / screen_height
}

/// The zoom whose tile resolution matches the view's, so tile-zoom
/// conventions (Nominatim's address depth) apply to a view with no tiles.
pub fn mercator_zoom(screen_height: f64) -> f64 {
    const TILE_PX: f64 = 256.0;
    libm::log2(epsg3857::U_TOTAL_X / TILE_PX / u_per_px(screen_height))
}

pub fn scale_bar(screen_height: f64) -> ScaleBar {
    let y_u = crate::control::camera::controller::map_center().map_or(0.0, |c| pvf3::to_real(pvf3::Y(&c)));
    let real_m_per_px = u_per_px(screen_height) / epsg3857::u_per_m(y_u);
    let total_meters = round_to_nice_scale(SCALE_BAR_MAX_WIDTH_PX * real_m_per_px);
    ScaleBar {
        total_meters,
        bar_width_px: if real_m_per_px > 0.0 { total_meters / real_m_per_px } else { 0.0 },
    }
}
