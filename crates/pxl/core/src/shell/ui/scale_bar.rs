// pxl_core/shell/ui/scale_bar.rs

//! The scale bar's reading: how many metres a bar of at most
//! SCALE_BAR_MAX_WIDTH_PX shows at the current distance and latitude, rounded to
//! the 1/2/5 steps the convention uses, and how wide that is. The shell draws
//! the bar and its label from this.

use pxl_shared::spec::space::adapter::epsg3857;

use crate::control::camera::config::camera_cfg;
use crate::control::camera::store::camera_store;
use crate::shell::map_store::map_store;

pub const SCALE_BAR_MAX_WIDTH_PX: f64 = 56.0;
pub const SCALE_BAR_TICK_PX: f64 = 6.0;
pub const SCALE_BAR_PAD_PX: f64 = 3.0;

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
    pub is_default_zoom: bool,
}

/// The scale bar for a viewport `screen_height` CSS pixels tall.
pub fn scale_bar(screen_height: f64) -> ScaleBar {
    let dist = camera_store().view_state.dist;
    let fov_rad = camera_cfg::projection::FOV.to_radians();
    let visible_u = 2.0 * (fov_rad / 2.0).tan() * dist;
    // `dist` is in canonical "u" units, not meters — the conversion is required.
    let visible_planar_m = visible_u / epsg3857::U_PER_M;
    let planar_m_per_px = visible_planar_m / screen_height;
    let mercator_scale = match map_store().map_center {
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
        is_default_zoom: (dist - camera_cfg::view::DEFAULT_DIST).abs() < 1.0,
    }
}
