// pxl-kernel/view/frame/resolution.rs

//! Device pixels per CSS pixel a surface is drawn at: the display's, and the
//! world's scaled by the load. The world's rays are cast at the display's
//! pixels times `scale` each way, and the page shows the buffer in the layout
//! box whatever its size (pxl-render gpu/surface.rs), so the frame is upscaled
//! within itself, never from the frames before: render resolution apart from
//! display resolution where a screen outgrows its GPU (Unreal's screen
//! percentage). Once a frame, where a GPU read landed, the cast's time stands
//! against the frame's budget, the paced interval the host grants (xpute host
//! sched/quantum.ts `Grant::paced_ms`): the scale moves by the square root of
//! budget over cast, as the cast's cost goes with its rays, between
//! `SCALE_MIN` and whole, and holds while the cast is within `SCALE_BAND` of
//! the budget. The read lags the frame it timed, so its time is taken per ray
//! it cast and projected to the rays cast now: a change still in flight is
//! not judged again.

use crate::param;
use pxl_render::api_spec::surface_role;
use pxl_render::gpu::surface::{surfaces, Surface};
use xpute_guest::global::Global;

/// The share of the display's pixels, each way, the world is drawn at.
static SCALE: Global<f64> = Global::new();

fn scale() -> &'static mut f64 {
    SCALE.get(|| 1.0)
}

fn world() -> Option<&'static mut Surface> {
    surfaces().find(|s| s.role == surface_role::WORLD)
}

/// Device pixels per CSS pixel the world is drawn at now: its surface's; a
/// pixel a CSS pixel before it has one.
pub fn world_ratio() -> f64 {
    world().map_or(1.0, |s| s.ratio)
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

/// A GPU read landed: the cast's `cast_ms` over the `cast_px` rays it cast,
/// against the frame's budget `paced_ms`. A frame that cast nothing, or has
/// no budget, judges nothing.
pub fn load(cast_ms: f64, cast_px: f64, paced_ms: f64) {
    let Some(s) = world() else { return };
    if !(cast_ms > 0.0 && cast_px > 0.0 && paced_ms > 0.0) {
        return;
    }
    let (w, h) = s.buffer_px();
    let now_ms = cast_ms * (f64::from(w) * f64::from(h)) / cast_px;
    let ratio = paced_ms / now_ms;
    if (ratio - 1.0).abs() <= param::SCALE_BAND {
        return;
    }
    let k = scale();
    *k = (*k * libm::sqrt(ratio)).clamp(param::SCALE_MIN, 1.0);
    s.ratio = surface_ratio(s.role, [s.px_w, s.px_h], s.dpr);
}
