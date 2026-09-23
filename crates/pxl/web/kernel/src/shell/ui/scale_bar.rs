// pxl-kernel/shell/ui/scale_bar.rs

//! What the view's scale reads as. The bar: how many metres a bar of at most
//! SCALE_BAR_MAX_WIDTH_PX shows at the current distance and latitude, rounded to
//! the 1/2/5 steps the convention uses, and how wide that is — the shell draws
//! the bar and its label from this. And the zoom the same resolution is, for
//! whatever is scaled the way a tiled map is.

use pxl_spec::space::adapter::epsg3857;

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
/// The Web Mercator metres one pixel covers: the projection's own units,
/// before the latitude a real distance needs.
fn planar_m_per_px(screen_height: f64) -> f64 {
    let dist = camera_store().view_state.dist;
    let fov_rad = camera_cfg::projection::FOV.to_radians();
    let visible_u = 2.0 * (fov_rad / 2.0).tan() * dist;
    // `dist` is in canonical "u" units, not meters — the conversion is required.
    visible_u / epsg3857::U_PER_M / screen_height
}

/// The Web Mercator zoom the view is at. A tile pyramid's level z has
/// `EQUATOR_M / (TILE_PX * 2^z)` planar metres a pixel, so the zoom is what
/// the view's own planar resolution reads as on that ladder — which is what
/// makes a convention written against tile zooms, like Nominatim's address
/// depth (pxl-spec place.rs), apply to a view that has no tiles.
pub fn mercator_zoom(screen_height: f64) -> f64 {
    const TILE_PX: f64 = 256.0;
    libm::log2(epsg3857::WORLD_WIDTH_3857 / TILE_PX / planar_m_per_px(screen_height))
}

pub fn scale_bar(screen_height: f64) -> ScaleBar {
    let dist = camera_store().view_state.dist;
    let planar_m_per_px = planar_m_per_px(screen_height);
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
