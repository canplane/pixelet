// pxl-kernel/view/frame/resolution.rs

//! Device pixels per CSS pixel a surface is drawn at: the display's, and the
//! world's scaled by the load. The world's rays are cast at the display's
//! pixels times `scale` each way, and the page shows the buffer in the layout
//! box whatever its size (pxl-render gpu/surface.rs), so the frame is upscaled
//! within itself, never from the frames before: render resolution apart from
//! display resolution where a screen outgrows its GPU (Unreal's screen
//! percentage). Once a frame, where a GPU read landed, the GPU time of its
//! passes drawn at the world's pixels (pxl-render gpu/renderer.rs
//! `pixels_ms`) stands against its budget, the paced interval the host grants
//! (xpute host sched/quantum.ts `Grant::paced_ms`): the scale moves by the
//! square root of budget over that time, as what those passes cost goes with
//! their pixels, between `SCALE_MIN` and whole, and holds while that time is
//! within `SCALE_BAND` of the budget. The read lags the frame it timed, so
//! only a frame drawn at the size drawn now is judged, and none while a change
//! waits to be drawn; a change asks for the frame that draws it.

use crate::param;
use pxl_render::api_spec::surface_role;
use pxl_render::gpu::surface::{surfaces, Surface};
use xpute_guest::global::Global;

/// The share of the display's pixels, each way, the world is drawn at.
static SCALE: Global<f64> = Global::new();

fn scale() -> &'static mut f64 {
    SCALE.get(|| 1.0)
}

/// The share of the display's pixels, each way, the world is drawn at now.
pub fn scale_now() -> f64 {
    *scale()
}

fn world() -> Option<&'static mut Surface> {
    surfaces().find(|s| s.role == surface_role::WORLD)
}

/// Device pixels per CSS pixel the world is drawn at now: its surface's; a
/// pixel a CSS pixel before it has one.
pub fn world_ratio() -> f64 {
    world().map_or(1.0, |s| s.ratio)
}

/// Device pixels per CSS pixel the display shows the world at, whatever the
/// load drew it at: what the camera grid is laid by (`view_resize`), so a
/// block's zoom, demand and a pointer's reach follow the screen, not the
/// frames before; a pixel a CSS pixel before the world has a surface.
pub fn display_ratio() -> f64 {
    world().map_or(1.0, |s| s.dpr.min(param::RESOLUTION_MAX))
}

/// The ratio a surface is drawn at: the display's, no finer than
/// `RESOLUTION_MAX`; the world's scaled by the load; a case's within its
/// thumbnail.
pub fn surface_ratio(role: u32, px: [u32; 2], dpr: f64) -> f64 {
    let device = dpr.min(param::RESOLUTION_MAX);
    match role {
        surface_role::WORLD => device * *scale(),
        surface_role::CASE => device.min(param::CASE_LONG_SIDE_MAX * dpr / f64::from(px[0].max(px[1]).max(1))),
        _ => device,
    }
}

/// A GPU read landed: `gpu_ms`, the time of its passes drawn at the world's
/// `gpu_px` pixels, against the frame's budget `paced_ms`. A frame that cast
/// nothing, has no budget, or was drawn at another size than now, judges
/// nothing.
pub fn load(gpu_ms: f64, gpu_px: f64, paced_ms: f64) {
    let Some(s) = world() else { return };
    if !(gpu_ms > 0.0 && gpu_px > 0.0 && paced_ms > 0.0) {
        return;
    }
    let drawn = f64::from(s.width) * f64::from(s.height);
    if s.buffer_px() != (s.width, s.height) || gpu_px != drawn {
        return;
    }
    let ratio = paced_ms / gpu_ms;
    if (ratio - 1.0).abs() <= param::SCALE_BAND {
        return;
    }
    let k = scale();
    *k = (*k * libm::sqrt(ratio)).clamp(param::SCALE_MIN, 1.0);
    s.ratio = surface_ratio(s.role, [s.px_w, s.px_h], s.dpr);
    crate::sched::edge::request_world_tick();
}
