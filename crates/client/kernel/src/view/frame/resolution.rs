// pxl-kernel/view/frame/resolution.rs

//! Device pixels per CSS pixel a surface is drawn at: the device's, so one
//! pixel drawn is one pixel shown. Nothing lowers it to buy frames; a frame
//! fits by its own structure.

use crate::param;
use pxl_render::gpu::api_spec::surface_role;
use pxl_render::gpu::surface::surfaces;

/// Device pixels per CSS pixel the world is drawn at now: its surface's.
pub fn world_ratio() -> f64 {
    surfaces().find(|s| s.role == surface_role::WORLD).map_or(param::RESOLUTION_MIN, |s| s.ratio)
}

/// The ratio a surface is drawn at once resized: the device's; a case's
/// within its thumbnail.
pub fn surface_ratio(role: u32, px: [u32; 2], dpr: f64) -> f64 {
    let device = dpr.clamp(param::RESOLUTION_MIN, param::RESOLUTION_MAX);
    match role {
        surface_role::CASE => device.min(param::CASE_LONG_SIDE_MAX * dpr / f64::from(px[0].max(px[1]).max(1))),
        _ => device,
    }
}
