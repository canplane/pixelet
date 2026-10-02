// pxl-kernel/shell/ui/scale_bar.rs

//! What the view's scale reads as: the scale bar (at most
//! SCALE_BAR_MAX_WIDTH_PX, rounded to 1/2/5 steps) and the equivalent Web
//! Mercator zoom.

use pxl_spec::space::adapter::epsg3857;

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

/// Web Mercator meters, before the latitude correction a real distance needs.
fn planar_m_per_px(screen_height: f64) -> f64 {
    let dist = crate::control::camera::controller::radius();
    let fov_rad = camera_cfg::projection::FOV.to_radians();
    let visible_u = 2.0 * (fov_rad / 2.0).tan() * dist;
    visible_u / epsg3857::U_PER_M / screen_height
}

/// The zoom whose tile resolution matches the view's, so tile-zoom
/// conventions (Nominatim's address depth) apply to a view with no tiles.
pub fn mercator_zoom(screen_height: f64) -> f64 {
    const TILE_PX: f64 = 256.0;
    libm::log2(epsg3857::WORLD_WIDTH_3857 / TILE_PX / planar_m_per_px(screen_height))
}

pub fn scale_bar(screen_height: f64) -> ScaleBar {
    let planar_m_per_px = planar_m_per_px(screen_height);
    let mercator_scale = match crate::control::camera::controller::map_center() {
        Some(c) => {
            let mut g = [0.0; 3];
            epsg3857::unproject(&mut g, c.i, c.f);
            epsg3857::Z_SCALE(g[1])
        }
        None => 1.0,
    };
    let real_m_per_px = planar_m_per_px / mercator_scale;
    let total_meters = round_to_nice_scale(SCALE_BAR_MAX_WIDTH_PX * real_m_per_px);
    ScaleBar {
        total_meters,
        bar_width_px: if real_m_per_px > 0.0 { total_meters / real_m_per_px } else { 0.0 },
    }
}
