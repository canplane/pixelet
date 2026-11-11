// pxl-core/door/apply/surface.rs

//! The surfaces the UI lends the GPU, opened, resized and closed by xpute's
//! GPU_SURFACE_OPEN, _RESIZE and _CLOSE.

use super::{or_refuse, refuse_unless};
use crate::domain::control::sensor;
use crate::domain::scene::resolution;
use crate::domain::view::frame::pose::view_resize;
use crate::sys::sched::request_world_tick;
use pxl_render::api_spec::surface_role;
use pxl_render::gpu::surface;
use xpute_core::io::door::Outcome;
use xpute_gpu::abi::{GpuSurfaceClose, GpuSurfaceResize};
use xpute_kit::status::errno::Errno;

/// Returns the new surface's handle, or 0 when none could be opened. The
/// surface draws nothing until SCENE_DRAW (scene.rs `draw`).
pub(super) fn open() -> Outcome {
    let handle = surface::surface_open();
    if handle != 0 {
        request_world_tick();
    }
    Ok(Some((handle as i32).into()))
}

/// Makes the surface `handle` draw the world. The sensors are watched while
/// the world's surface is open, and the view is sized at once when the
/// surface's box is already known. Returns false when the surface is not
/// open.
pub(super) fn draw_world(handle: u32) -> bool {
    let Some(s) = surface::surface(handle) else {
        return false;
    };
    s.role = surface_role::WORLD;
    sensor::sensors_open();
    if s.px_w != 0 {
        s.ratio = resolution::surface_ratio(s.role, [s.px_w, s.px_h], s.dpr);
        view_resize(f64::from(s.px_w) / s.dpr, f64::from(s.px_h) / s.dpr);
    }
    request_world_tick();
    true
}

pub(super) fn resize(c: GpuSurfaceResize) -> Outcome {
    refuse_unless(c.device_pixel_ratio > 0.0, Errno::EINVAL)?;
    let role = surface::surface(c.surface).map_or(0, |s| s.role);
    let ratio = resolution::surface_ratio(role, [c.width, c.height], c.device_pixel_ratio);
    refuse_unless(
        surface::surface_resize(c.surface, c.width, c.height, c.device_pixel_ratio, ratio),
        Errno::EBADF,
    )?;
    // The world's view is its surface's layout box, in CSS pixels.
    if role == surface_role::WORLD {
        view_resize(
            f64::from(c.width) / c.device_pixel_ratio,
            f64::from(c.height) / c.device_pixel_ratio,
        );
    }
    request_world_tick();
    Ok(Some(0.into()))
}

pub(super) fn close(c: GpuSurfaceClose) -> Outcome {
    let role = or_refuse(surface::surface(c.surface).map(|s| s.role), Errno::EBADF)?;
    super::super::close_surface(c.surface);
    // A surface can close after the one replacing it has opened, so the
    // sensors stop with the last world's surface.
    let world_open = surface::surfaces().any(|s| s.role == surface_role::WORLD);
    if role == surface_role::WORLD && !world_open {
        sensor::sensors_close();
    }
    Ok(Some(0.into()))
}
